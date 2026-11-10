//! The top-level emulation config.

use serde::{Deserialize, Serialize};
use wreq::header::{COOKIE, HeaderMap, HeaderValue};

use crate::{
    cookies::CookieConfig,
    headers::{OrigHeaders, header_pairs},
    http1::Http1Config,
    http2::Http2Config,
    tls::TlsConfig,
};

/// Everything a `wreq::Emulation` holds, as plain data.
///
/// Missing keys fall back to empty values and unknown keys are rejected, as with the fork.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EmulationConfig {
    /// Headers added to every request.
    #[serde(with = "header_pairs")]
    pub headers: HeaderMap,
    /// The order headers are sent in, with their original casing.
    pub orig_headers: OrigHeaders,
    /// TLS, HTTP/1 and HTTP/2 options.
    pub transport: TransportConfig,
    /// How the cookies from a cookie store go out. The JSON leaves out the default, so a config
    /// that does not set it reads and writes as it did before this field existed.
    #[serde(skip_serializing_if = "CookieConfig::is_default")]
    pub cookies: CookieConfig,
}

/// The protocol options of an [`EmulationConfig`]. Each part is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportConfig {
    pub tls_options: Option<TlsConfig>,
    pub http1_options: Option<Http1Config>,
    pub http2_options: Option<Http2Config>,
}

impl EmulationConfig {
    /// Returns a copy with a JSON merge patch (RFC 7386) applied to its wire form.
    ///
    /// Objects merge key by key, any other value replaces what was there, and `null` removes a
    /// key (which then reads back as that key's default). This lets you change a few fields of a
    /// profile without copying all of it:
    ///
    /// ```
    /// use emulation_config::Profile;
    ///
    /// let patch = serde_json::json!({
    ///     "transport": { "tls_options": { "record_size_limit": 16385 } }
    /// });
    /// let config = Profile::Chrome145Macos.config().patched(&patch)?;
    /// # let _ = config;
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Fails if the patched JSON is not a valid config, for example when it removes a required
    /// HTTP/2 field.
    pub fn patched(&self, patch: &serde_json::Value) -> serde_json::Result<Self> {
        let mut value = serde_json::to_value(self)?;
        merge_patch(&mut value, patch);
        serde_json::from_value(value)
    }

    /// Moves a `cookie` entry from [`headers`](Self::headers) into the header order, and returns
    /// the value the entry held.
    ///
    /// wreq asks a cookie store only when a request has no `Cookie` header. It puts the store's
    /// cookies in the order's `cookie` slot, or after every other header when the order has none.
    /// Over HTTP/2 the store can send one `cookie` field per cookie, as browsers do, which a
    /// `Cookie` header in `headers` cannot. After this call the store's cookies go where the entry
    /// was.
    ///
    /// An order that already lists `cookie` keeps its slot. Otherwise `cookie` goes right after the
    /// last header that comes before it in `headers` and is in the order. An empty order gets the
    /// headers before `cookie` in their `headers` order, so no other header moves.
    pub fn move_cookie_slot_to_header_order(&mut self) -> Option<HeaderValue> {
        let position = self.headers.keys().position(|name| name == COOKIE)?;
        let preceding = self
            .headers
            .keys()
            .take(position)
            .cloned()
            .collect::<Vec<_>>();
        let value = self.headers.remove(COOKIE);
        if self.orig_headers.is_empty() {
            self.orig_headers = preceding.into_iter().chain([COOKIE]).collect();
        } else if !self.orig_headers.contains(&COOKIE) {
            self.orig_headers.insert_after_last_of(&preceding, COOKIE);
        }
        value
    }
}

/// Applies `patch` to `target` as described in RFC 7386, section 2.
fn merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    let serde_json::Value::Object(patch) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = serde_json::Value::Object(serde_json::Map::new());
    }
    let serde_json::Value::Object(target) = target else {
        unreachable!("target was just made an object");
    };
    for (key, value) in patch {
        if value.is_null() {
            target.remove(key);
        } else {
            merge_patch(
                target
                    .entry(key.as_str())
                    .or_insert(serde_json::Value::Null),
                value,
            );
        }
    }
}

impl From<&EmulationConfig> for wreq::Emulation {
    fn from(config: &EmulationConfig) -> Self {
        let mut builder = Self::builder()
            .headers(config.headers.clone())
            .orig_headers((&config.orig_headers).into());

        let transport = &config.transport;
        if let Some(tls) = &transport.tls_options {
            builder = builder.tls_options(tls.into());
        }
        if let Some(http1) = &transport.http1_options {
            builder = builder.http1_options(http1.into());
        }
        if let Some(http2) = &transport.http2_options {
            builder = builder.http2_options(http2.into());
        }
        // A config carries no connection group; requests get wreq's default one.
        builder.build(wreq::Group::default())
    }
}

/// Lets wreq's `emulation(...)` methods (on `ClientBuilder`, `RequestBuilder` and the WebSocket
/// builder) take a config directly: wreq's `IntoEmulation` is implemented for every
/// `Into<Emulation>`.
impl From<EmulationConfig> for wreq::Emulation {
    fn from(config: EmulationConfig) -> Self {
        Self::from(&config)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use wreq::header::{ACCEPT, COOKIE, HeaderName, HeaderValue, USER_AGENT};

    use super::merge_patch;
    use crate::{EmulationConfig, OrigHeaderName, OrigHeaders, Profile};

    fn order_names(config: &EmulationConfig) -> Vec<String> {
        config
            .orig_headers
            .iter()
            .map(|name| match name {
                OrigHeaderName::Cased(bytes) => String::from_utf8_lossy(bytes).into_owned(),
                OrigHeaderName::Standard(name) => name.as_str().to_owned(),
            })
            .collect()
    }

    #[test]
    fn a_cookie_entry_becomes_a_slot_in_an_empty_order() {
        // A `user-agent` and an empty `cookie` entry, and no order: an empty entry marks the slot.
        let mut config = EmulationConfig::default();
        config
            .headers
            .insert(USER_AGENT, HeaderValue::from_static("ua"));
        config.headers.insert(COOKIE, HeaderValue::from_static(""));
        let value = config.move_cookie_slot_to_header_order();
        assert_eq!(value, Some(HeaderValue::from_static("")));
        assert!(!config.headers.contains_key(COOKIE));
        assert!(config.headers.contains_key(USER_AGENT));
        assert_eq!(order_names(&config), ["user-agent", "cookie"]);
    }

    #[test]
    fn an_order_that_lists_cookie_keeps_its_slot() {
        let mut config = EmulationConfig::default();
        config
            .headers
            .insert(COOKIE, HeaderValue::from_static("a=1"));
        config.orig_headers = [
            USER_AGENT,
            ACCEPT,
            COOKIE,
            HeaderName::from_static("priority"),
        ]
        .into_iter()
        .collect::<OrigHeaders>();
        let order = config.orig_headers.clone();
        let value = config.move_cookie_slot_to_header_order();
        assert_eq!(value, Some(HeaderValue::from_static("a=1")));
        assert_eq!(config.orig_headers, order);
    }

    #[test]
    fn cookie_goes_after_the_last_listed_header_before_it() {
        let mut config = EmulationConfig::default();
        config
            .headers
            .insert(USER_AGENT, HeaderValue::from_static("ua"));
        config.headers.insert(
            HeaderName::from_static("x-unlisted"),
            HeaderValue::from_static("1"),
        );
        config.headers.insert(COOKIE, HeaderValue::from_static(""));
        config.orig_headers = [ACCEPT, USER_AGENT, HeaderName::from_static("priority")]
            .into_iter()
            .collect::<OrigHeaders>();
        config.move_cookie_slot_to_header_order();
        assert_eq!(
            order_names(&config),
            ["accept", "user-agent", "cookie", "priority"]
        );
    }

    #[test]
    fn a_config_without_a_cookie_entry_is_unchanged() {
        let mut config = Profile::Chrome145Macos.config();
        let order = config.orig_headers.clone();
        assert_eq!(config.move_cookie_slot_to_header_order(), None);
        assert_eq!(config.orig_headers, order);
    }

    #[test]
    fn merge_patch_matches_rfc_7386_appendix_a() {
        // Every example from RFC 7386, Appendix A: (original, patch, result).
        let cases: [(Value, Value, Value); 15] = [
            (json!({"a":"b"}), json!({"a":"c"}), json!({"a":"c"})),
            (json!({"a":"b"}), json!({"b":"c"}), json!({"a":"b","b":"c"})),
            (json!({"a":"b"}), json!({"a":null}), json!({})),
            (
                json!({"a":"b","b":"c"}),
                json!({"a":null}),
                json!({"b":"c"}),
            ),
            (json!({"a":["b"]}), json!({"a":"c"}), json!({"a":"c"})),
            (json!({"a":"c"}), json!({"a":["b"]}), json!({"a":["b"]})),
            (
                json!({"a":{"b":"c"}}),
                json!({"a":{"b":"d","c":null}}),
                json!({"a":{"b":"d"}}),
            ),
            (json!({"a":[{"b":"c"}]}), json!({"a":[1]}), json!({"a":[1]})),
            (json!(["a", "b"]), json!(["c", "d"]), json!(["c", "d"])),
            (json!({"a":"b"}), json!(["c"]), json!(["c"])),
            (json!({"a":"foo"}), json!(null), json!(null)),
            (json!({"a":"foo"}), json!("bar"), json!("bar")),
            (json!({"e":null}), json!({"a":1}), json!({"e":null,"a":1})),
            (json!([1, 2]), json!({"a":"b","c":null}), json!({"a":"b"})),
            (
                json!({}),
                json!({"a":{"bb":{"ccc":null}}}),
                json!({"a":{"bb":{}}}),
            ),
        ];
        for (mut original, patch, result) in cases {
            merge_patch(&mut original, &patch);
            assert_eq!(original, result, "patch: {patch}");
        }
    }
}
