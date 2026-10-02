//! The top-level emulation config.

use serde::{Deserialize, Serialize};
use wreq::header::HeaderMap;

use crate::{
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
        builder.build()
    }
}

/// Lets wreq's `emulation(...)` methods (on `ClientBuilder`, `RequestBuilder` and the WebSocket
/// builder) take a config directly.
impl wreq::EmulationFactory for EmulationConfig {
    fn emulation(self) -> wreq::Emulation {
        wreq::Emulation::from(&self)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::merge_patch;

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
