//! How a client sends the cookies from its cookie store.
//!
//! Browsers differ here, and a server can see it. Chrome 156 and Firefox 157 both send one
//! `cookie` field per cookie over HTTP/2 and one combined field over HTTP/1.1, and both sort
//! longest path first, then oldest first. They differ when a response sets a cookie again with a
//! new value, and in how HPACK encodes short cookie fields.

use serde::{Deserialize, Serialize};
use wreq::{Version, cookie::Cookies, header::HeaderValue};

use crate::string_enum::string_enum;

string_enum! {
    /// How cookies go out over HTTP/2 and HTTP/3. HTTP/1.x always gets one combined field, as
    /// RFC 6265, section 5.4 requires.
    pub enum CookieLayout("cookie layout") {
        /// One `cookie` field with every pair, joined by `"; "`.
        Combined => "combined",
        /// One `cookie` field per pair (RFC 9113, section 8.2.3), as Chrome and Firefox send them.
        Split => "split",
    }
}

string_enum! {
    /// What happens to a cookie's place in the order when a response sets it again.
    pub enum CookieOverwrite("cookie overwrite rule") {
        /// The cookie keeps its creation time, so it keeps its place. RFC 6265, section 5.3,
        /// step 11.3, and Firefox.
        KeepCreationTime => "keep_creation_time",
        /// The cookie keeps its creation time only when the value is the same. A new value makes
        /// it a new cookie, which goes after the others with a path of the same length. Chrome.
        RenewOnNewValue => "renew_on_new_value",
    }
}

impl CookieOverwrite {
    /// Whether a cookie set again with `new_value` gets a new creation time.
    #[must_use]
    pub fn renews_creation_time(self, old_value: &str, new_value: &str) -> bool {
        match self {
            Self::KeepCreationTime => false,
            Self::RenewOnNewValue => old_value != new_value,
        }
    }
}

/// How a client lays out, orders and encodes the cookies from its cookie store.
///
/// The default is one combined field, as the SDK sent before this setting existed, with the
/// RFC 6265 order. Unknown settings are ignored, so a reader accepts a config from a newer writer
/// that adds one. An unknown value of a known setting is an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CookieConfig {
    /// One field, or one field per cookie, over HTTP/2 and HTTP/3.
    pub layout: CookieLayout,
    /// Whether a cookie set again moves to the end of its group.
    pub overwrite: CookieOverwrite,
    /// Over HTTP/2 and HTTP/3, a cookie field shorter than this many bytes (`name=value`) goes
    /// out as an HPACK "never indexed" literal. Firefox uses 20, Chrome `None`.
    ///
    /// Every other cookie field goes out as http2 0.5.20 encodes cookies: a literal without
    /// indexing, on every request. Chrome and Firefox add those fields to the HPACK table and
    /// then send an index. http2 decides that in `Header::skip_value_index`, so matching it needs
    /// a change there.
    pub never_index_below: Option<u16>,
}

impl Default for CookieConfig {
    fn default() -> Self {
        Self {
            layout: CookieLayout::Combined,
            overwrite: CookieOverwrite::KeepCreationTime,
            never_index_below: None,
        }
    }
}

impl CookieConfig {
    /// Chrome, measured on Chrome 156.
    pub const CHROME: Self = Self {
        layout: CookieLayout::Split,
        overwrite: CookieOverwrite::RenewOnNewValue,
        never_index_below: None,
    };

    /// Firefox, measured on Firefox 157.
    pub const FIREFOX: Self = Self {
        layout: CookieLayout::Split,
        overwrite: CookieOverwrite::KeepCreationTime,
        never_index_below: Some(20),
    };

    /// Whether this is the default, which the JSON leaves out.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The `cookie` fields for a request over `version`.
    ///
    /// Each item of `pairs` is one `name=value` pair, already in the order to send. An empty
    /// pair, or one that is not a valid header value, is skipped.
    pub fn request_cookies<I>(&self, pairs: I, version: Version) -> Cookies
    where
        I: IntoIterator,
        I::Item: AsRef<[u8]>,
    {
        let hpack = matches!(version, Version::HTTP_2 | Version::HTTP_3);
        let pairs = pairs.into_iter().filter(|pair| !pair.as_ref().is_empty());

        if hpack && self.layout == CookieLayout::Split {
            let fields = pairs
                .filter_map(|pair| self.field(pair.as_ref()))
                .collect::<Vec<_>>();
            return if fields.is_empty() {
                Cookies::Empty
            } else {
                Cookies::Uncompressed(fields)
            };
        }

        let joined = pairs.fold(Vec::new(), |mut joined, pair| {
            if !joined.is_empty() {
                joined.extend_from_slice(b"; ");
            }
            joined.extend_from_slice(pair.as_ref());
            joined
        });
        if joined.is_empty() {
            return Cookies::Empty;
        }
        if hpack {
            self.field(&joined)
                .map_or(Cookies::Empty, Cookies::Compressed)
        } else {
            HeaderValue::from_bytes(&joined).map_or(Cookies::Empty, Cookies::Compressed)
        }
    }

    /// One field for HTTP/2 or HTTP/3, marked sensitive when it is under `never_index_below`.
    /// HPACK sends a sensitive value as a "never indexed" literal.
    fn field(self, bytes: &[u8]) -> Option<HeaderValue> {
        let mut value = HeaderValue::from_bytes(bytes).ok()?;
        let limit = self.never_index_below.map(usize::from);
        if limit.is_some_and(|limit| bytes.len() < limit) {
            value.set_sensitive(true);
        }
        Some(value)
    }
}

/// The `name=value` pairs of a `Cookie` header value, in their order.
///
/// Pairs are separated by `;` with optional spaces or tabs around them, as Chrome's HPACK encoder
/// splits a header into crumbs. Empty pairs are skipped.
pub fn cookie_pairs(header: &[u8]) -> impl Iterator<Item = &[u8]> {
    header
        .split(|&byte| byte == b';')
        .map(<[u8]>::trim_ascii)
        .filter(|pair| !pair.is_empty())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use wreq::{Version, cookie::Cookies};

    use super::{CookieConfig, CookieLayout, CookieOverwrite, cookie_pairs};

    /// The fields of the last probe in the `order` capture: after `z` was set again with a new
    /// value. Chrome sent `b=2`, `a=3`, `z=9`.
    const CHROME_LAST_PROBE: [&str; 3] = ["b=2", "a=3", "z=9"];

    /// The fields of the `hpack` capture: 3, 19 and 20 bytes long.
    const HPACK_FIELDS: [&str; 3] = ["s=1", "n19=0123456789abcde", "n20=0123456789abcdef"];

    fn fields(cookies: &Cookies) -> Vec<(String, bool)> {
        match cookies {
            Cookies::Uncompressed(values) => values
                .iter()
                .map(|value| {
                    let text = value.to_str().unwrap().to_owned();
                    (text, value.is_sensitive())
                })
                .collect(),
            other => panic!("expected one field per cookie, got {other:?}"),
        }
    }

    fn combined(cookies: &Cookies) -> (String, bool) {
        match cookies {
            Cookies::Compressed(value) => {
                (value.to_str().unwrap().to_owned(), value.is_sensitive())
            }
            other => panic!("expected one combined field, got {other:?}"),
        }
    }

    #[test]
    fn split_sends_one_field_per_cookie_over_http2_and_http3() {
        for version in [Version::HTTP_2, Version::HTTP_3] {
            let cookies = CookieConfig::CHROME.request_cookies(CHROME_LAST_PROBE, version);
            let expected = CHROME_LAST_PROBE.map(|field| (field.to_owned(), false));
            assert_eq!(fields(&cookies), expected, "{version:?}");
        }
    }

    #[test]
    fn http1_always_gets_one_combined_field() {
        // Chrome sent `b=2; a=3; z=9` over HTTP/1.1 for the same probe.
        for config in [
            CookieConfig::CHROME,
            CookieConfig::FIREFOX,
            CookieConfig::default(),
        ] {
            for version in [Version::HTTP_10, Version::HTTP_11] {
                let cookies = config.request_cookies(CHROME_LAST_PROBE, version);
                assert_eq!(combined(&cookies), ("b=2; a=3; z=9".to_owned(), false));
            }
        }
    }

    #[test]
    fn the_default_combines_over_http2_too() {
        let cookies = CookieConfig::default().request_cookies(CHROME_LAST_PROBE, Version::HTTP_2);
        assert_eq!(combined(&cookies), ("b=2; a=3; z=9".to_owned(), false));
    }

    #[test]
    fn firefox_never_indexes_the_fields_it_sent_never_indexed() {
        // Wireshark showed `s=1` and `n19=…` as "never Indexed" on every request, and `n20=…`
        // as indexed.
        let cookies = CookieConfig::FIREFOX.request_cookies(HPACK_FIELDS, Version::HTTP_2);
        let expected = [
            ("s=1".to_owned(), true),
            ("n19=0123456789abcde".to_owned(), true),
            ("n20=0123456789abcdef".to_owned(), false),
        ];
        assert_eq!(fields(&cookies), expected);
    }

    #[test]
    fn chrome_marks_no_field_never_indexed() {
        let cookies = CookieConfig::CHROME.request_cookies(HPACK_FIELDS, Version::HTTP_2);
        assert!(fields(&cookies).iter().all(|(_, sensitive)| !sensitive));
    }

    #[test]
    fn the_limit_does_not_apply_over_http1() {
        let cookies = CookieConfig::FIREFOX.request_cookies(["s=1"], Version::HTTP_11);
        assert_eq!(combined(&cookies), ("s=1".to_owned(), false));
    }

    #[test]
    fn no_pairs_means_no_field() {
        let no_pairs: [&str; 0] = [];
        for config in [CookieConfig::CHROME, CookieConfig::default()] {
            for version in [Version::HTTP_11, Version::HTTP_2] {
                let cookies = config.request_cookies(no_pairs, version);
                assert!(matches!(cookies, Cookies::Empty), "{config:?} {version:?}");
                let cookies = config.request_cookies([""], version);
                assert!(matches!(cookies, Cookies::Empty), "{config:?} {version:?}");
            }
        }
    }

    #[test]
    fn an_invalid_pair_is_skipped() {
        let cookies = CookieConfig::CHROME.request_cookies(["a=1", "b=\n", "c=3"], Version::HTTP_2);
        assert_eq!(
            fields(&cookies),
            [("a=1".to_owned(), false), ("c=3".to_owned(), false)]
        );
    }

    #[test]
    fn cookie_pairs_splits_a_combined_header_in_order() {
        let pairs = cookie_pairs(b"b=2; a=3;z=9 ;\t; ")
            .map(|pair| std::str::from_utf8(pair).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(pairs, ["b=2", "a=3", "z=9"]);
    }

    #[test]
    fn overwrite_rules_match_the_captured_orders() {
        // `z=1` set again as `z=1` kept its place in both browsers. Set again as `z=9`, it moved
        // to the end in Chrome and kept its place in Firefox.
        let chrome = CookieConfig::CHROME.overwrite;
        let firefox = CookieConfig::FIREFOX.overwrite;
        assert!(!chrome.renews_creation_time("1", "1"));
        assert!(chrome.renews_creation_time("1", "9"));
        assert!(!firefox.renews_creation_time("1", "1"));
        assert!(!firefox.renews_creation_time("1", "9"));
    }

    #[test]
    fn json_names_every_setting() {
        let json = serde_json::to_string(&CookieConfig::FIREFOX).unwrap();
        assert_eq!(
            json,
            r#"{"layout":"split","overwrite":"keep_creation_time","never_index_below":20}"#
        );
        let read: CookieConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(read, CookieConfig::FIREFOX);
    }

    #[test]
    fn missing_settings_take_the_default() {
        let read: CookieConfig = serde_json::from_str(r#"{"layout":"split"}"#).unwrap();
        assert_eq!(
            read,
            CookieConfig {
                layout: CookieLayout::Split,
                ..CookieConfig::default()
            }
        );
    }

    #[test]
    fn unknown_settings_are_ignored() {
        let read: CookieConfig =
            serde_json::from_str(r#"{"layout":"split","order":"name"}"#).unwrap();
        assert_eq!(
            read,
            CookieConfig {
                layout: CookieLayout::Split,
                ..CookieConfig::default()
            }
        );
    }

    #[test]
    fn unknown_values_are_rejected() {
        for json in [
            r#"{"layout":"per_cookie"}"#,
            r#"{"overwrite":"renew"}"#,
            r#"{"never_index_below":-1}"#,
        ] {
            assert!(
                serde_json::from_str::<CookieConfig>(json).is_err(),
                "{json}"
            );
        }
        assert_eq!(
            "renew".parse::<CookieOverwrite>().unwrap_err().to_string(),
            "unknown cookie overwrite rule: `renew`"
        );
    }
}
