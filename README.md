# emulation-config

Plain-data emulation configs for [wreq](https://github.com/OpacityLabs/wreq): default headers,
header order, TLS, HTTP/1 and HTTP/2 options, and how cookies go out. It ships the browser presets
published by the wreq project as built-in profiles, and every field can be changed.

The JSON format is the SDK's wire format. This crate reads and writes it byte for byte
like the serde code that lived in the `OpacityLabs/wreq` fork (`v6.0.0-rc.28`) and
`OpacityLabs/http2` (`v0.5.11`). The format is defined here rather than inside wreq, so wreq's
internals can change without changing what goes over the wire, and wreq itself needs no serde.

## Usage

Start from a profile, a JSON config, or an empty config, then convert to wreq:

```rust
use emulation_config::{EmulationConfig, Profile};

// A built-in profile, picked by name.
let config = "safari_ios_26_2".parse::<Profile>()?.config();

// Or any config in the wire format.
let config: EmulationConfig = serde_json::from_slice(&body)?;

let client = wreq::Client::builder().emulation(config).build()?;
```

`EmulationConfig` converts into `wreq::Emulation`, and wreq's `IntoEmulation` covers every such
type, so every wreq `emulation(...)` method takes it. `wreq::Emulation::from(&config)` gives the
wreq value itself.

Two ways to customize:

```rust
// In code: every field is public.
let mut config = Profile::Chrome145Macos.config();
config.transport.tls_options.as_mut().unwrap().record_size_limit = Some(16385);

// As JSON (handy from Python or a config file): an RFC 7386 merge patch.
let config = Profile::Chrome145Macos.config().patched(&serde_json::json!({
    "transport": { "tls_options": { "record_size_limit": 16385 } }
}))?;
```

wreq's builder ties `adaptive_window` to the two HTTP/2 window sizes: turning it on resets both
windows to 65,535, and setting either window turns it off. `Http2Config::set_adaptive_window`,
`set_initial_window_size` and `set_initial_conn_window_size` apply those rules. Writing the
fields directly, or patching them, stores the values as given, so set all three together.

`TlsConfig`, `Http1Config` and `Http2Config` also convert on their own into `TlsOptions`,
`Http1Options` and `Http2Options`. Conversion goes one way, from config to wreq.

`TlsConfig::default()`, `Http1Config::default()` and `Http2Config::default()` equal wreq's own
defaults, with one exception: wreq 0.16 defaults `max_frame_size` and `max_header_list_size` to
16384 and sends both in the SETTINGS frame, while `Http2Config::default()` leaves them unset, as
the fork did, so a config that omits them sends the same frame as before. `PseudoOrder::new`,
`SettingsOrder::new`, `Priorities::new` and `ExperimentalSettings::new` clean up their input the
same way wreq's builders do, and the `Http2Config` window setters apply the builder's window
rules. So a config built in code produces the same JSON the wreq builders produce. The SDK's Lua
parser builds configs this way.

## Versions and wreq

This crate has its own version. It pins wreq in `Cargo.toml` (and the exact commit in
`Cargo.lock`):

| emulation-config | wreq |
|---|---|
| 0.1.x | `OpacityLabs/wreq` `v6.0.0-rc.28.opacity.1`: no serde, http2 0.5.11 from crates.io |
| 0.2.x | `OpacityLabs/wreq` `v6.0.0-rc.28.opacity.2`: `ClientBuilder::tcp_stream` in place of `socket` |
| 0.3.x | `OpacityLabs/wreq` `v0.16.1-opacity.1`: upstream `v0.16.1` (btls, wreq-proto, http2 0.5.20). wreq needs its `tokio-rt` feature |
| 0.4.x | `OpacityLabs/wreq` `v0.16.3-opacity.1`: upstream `v0.16.3`. wreq needs its `tokio-rt` feature |
| 0.5.x | `OpacityLabs/wreq` `v0.16.3-opacity.1`, as 0.4.x. wreq needs its `tokio-rt` and `cookies` features |

[CHANGELOG.md](CHANGELOG.md) records, for every release, the wreq it pins, whether the wire
format changed, and which profiles changed.

A project that uses this crate must use the same wreq. If it pins a different wreq commit, Cargo
builds two copies of wreq and `wreq::Emulation::from(&config)` does not compile, so a mismatch
can't go unnoticed. Moving to a new wreq means a new release of this crate that pins it.

## Profiles

The built-in profiles are the browser presets published by the wreq project (wreq-util
3.0.0-rc.10):

| Name | Client |
|---|---|
| `chrome_145_macos`, `chrome_145_windows`, `chrome_145_android` | Chrome 145 |
| `firefox_147_macos`, `firefox_147_windows` | Firefox 147 |
| `firefox_android_135` | Firefox 135, Android |
| `safari_26_2`, `safari_ios_26_2`, `safari_ipad_26_2` | Safari 26.2, macOS / iPhone / iPad |

The wreq-util presets were not captured by us, so compare them against a capture of the real
browser (for example from tls.peet.ws) before relying on one.

Each profile is listed in the `profiles!` macro in `src/profiles.rs`. A test fails until that
list and the `profiles/` folder match.

The `cookies` setting is ours, not part of the wreq-util presets. The Chrome profiles use
`CookieConfig::CHROME` and the Firefox profiles `CookieConfig::FIREFOX`, measured on Chrome 156
and Firefox 157. The Safari profiles keep the default.

## Cookies

wreq takes the cookies for a request from a `wreq::cookie::CookieStore`, not from the
`Emulation`. `EmulationConfig::cookies` describes how the client being emulated sends them, and
the cookie store applies it:

| Setting | Default | Chrome | Firefox |
|---|---|---|---|
| `layout` over HTTP/2 and HTTP/3 | `combined`: one field | `split`: one field per cookie | `split` |
| `overwrite`: a cookie set again with a new value | `keep_creation_time`: keeps its place | `renew_on_new_value`: moves to the end | `keep_creation_time` |
| `never_index_below`: HPACK "never indexed" for shorter fields | `null` | `null` | `20` |

Over HTTP/1.x every setting gives one combined field, as RFC 6265 requires.

One HPACK difference stays. http2 0.5.20 never adds a cookie field to the HPACK table
(`Header::skip_value_index`, a rule taken from nghttp2), so a field that is not marked "never
indexed" goes out as a literal without indexing on every request. Chrome adds every cookie field
to the table on its first request and then sends an index, and Firefox does the same for fields
of 20 bytes or more. A capture of wreq with the Chrome and Firefox profiles shows this. Matching
the browsers needs a change in http2. Both browsers sort
longest path first, then oldest first. The store sorts, and calls
`CookieOverwrite::renews_creation_time` when a response sets a cookie again, then
`CookieConfig::request_cookies` to build the fields. `EmulationConfig::move_cookie_slot_to_header_order`
puts the fields in their place in the header order.

## Wire format

Decoded, a payload looks like this (values shortened):

```text
{
  "headers":      [[<"user-agent" as bytes>, <value as bytes>], ...],
  "orig_headers": [[0, <"User-Agent" as bytes>], [1, <"host" as bytes>], ...],
  "transport": {
    "tls_options":   { "alpn_protocols": ["h2", "http/1.1"], "prefer_chacha20": null, ... },
    "http1_options": null,
    "http2_options": { "headers_pseudo_order": { "ids": ["METHOD", ...] },
                       "keep_alive_timeout": { "secs": 20, "nanos": 0 }, ... }
  }
}
```

- Header names and values are lists of byte values, not strings.
- In `orig_headers`, the first byte is a tag: `0` = send with this exact casing, `1` = standard
  lowercase name.
- `prefer_chacha20` is the wire name of wreq's `preserve_tls13_cipher_list`.
- A missing `alpn_protocols` means no ALPN at all. This is the one missing key whose value
  differs from wreq's default (`["h2", "http/1.1"]`).
- `experimental_settings` is dead. wreq 0.16 has no such option, and before that the format
  could only name standard settings, which wreq dropped from this list, so the SDK always sends
  it empty. It is still read and written, and the conversion ignores it.
- `key_shares_limit` is the number of leading `curves_list` groups that get a key share. wreq
  0.16 names the groups instead, so the conversion takes the first N entries of `curves_list`,
  the mapping upstream's own Firefox presets use. With no `curves_list` the limit does nothing.
- Unknown keys are ignored in every object except a duration (`{"secs": .., "nanos": ..}`), so
  a key that a newer writer adds does not fail a reader on 0.5.0 or later. The reader drops that
  key and its setting. A reader on 0.4.x or older, like the fork, rejects an unknown top-level
  key. An unknown value of a known key, such as a new `layout` name, is still an error.
- `cookies` is left out when it is the default (`{"layout":"combined","overwrite":
  "keep_creation_time","never_index_below":null}`), so a config that does not set it writes the
  same JSON as before. A reader on 0.4.x or older rejects a payload with the key.
- `key_shares` names the groups directly (`["X25519MLKEM768", "X25519"]`, as in `curves_list`)
  and wins over `key_shares_limit`. The fork had no such key, so it is left out of the JSON when
  unset, and a reader on 0.3.x or older ignores it.

## How the format is checked

The reference for the wire format is the fork's serde (`OpacityLabs/wreq` tag `v6.0.0-rc.28`),
recorded in `tests/golden/`. No expected output is written by hand, and the tests never call the
fork's serde, so they keep working now that the code is gone from wreq.

- `tests/golden/wire_cases.json`: 55 inputs (edge cases, rejected inputs, every enum value) with
  what the fork wrote back or whether it rejected them. Recorded cases are never edited or
  removed, only added: CI fails if a case's `input` or `fork_output` differs from the base
  branch (`.github/scripts/goldens_only_grow.py`).
- `tests/golden/wreq_debug.json`: `Debug` of the wreq value each profile and each accepted case
  converts to. This is a snapshot of the crate's own conversion, not a recording of the fork.
  `UPDATE_WREQ_DEBUG=1 cargo test` rewrites it; the diff is reviewed with the change that caused
  it, and a change that alters what goes on the wire is a Breaking entry.

A diff under `profiles/` or `tests/golden/` means a profile's fingerprint, the wire format or a
conversion changed, so it comes with an entry in [CHANGELOG.md](CHANGELOG.md).

The tests check that:
- every profile reproduces byte for byte through this crate's serde and `Profile::config()`;
- every recorded case gives the fork's output, or is rejected like the fork rejected it;
- a key added to every object of every profile, except a duration, is ignored;
- every profile and every accepted case converts to the wreq value in the snapshot;
- the zlib, brotli and zstd certificate compressors round-trip and report their algorithm;
- defaults, the builder-style constructors and the window setters match wreq's own (the setters
  against every sequence of up to three window calls on wreq's builder);
- past the most distinct header names an `http::HeaderMap` holds, reading returns an error
  rather than panicking;
- the checked number types (`StreamId`, `InitialStreamId`, `MaxFrameSize`, `ConnWindowSize`,
  `MaxSendBufferSize`, `MaxBufSize`) reject exactly the values wreq and http2 panic on. The test
  runs values on each side of every limit through their real code, so an upgrade that changes a
  limit makes it fail. `KeepAliveDuration` has a round limit of one day instead, and the test
  checks that `Instant + Duration`, which wreq's keep-alive code uses, still panics near
  `Duration::MAX` and not at that limit;
- the cookie presets give the field layout, overwrite rule and HPACK marking captured from
  Chrome 156 and Firefox 157, and every browser profile carries its preset;
- `patched()` matches making the same change in code, and the merge logic passes every example
  in RFC 7386, Appendix A.

## Known differences from the fork

No SDK flow produces any of these, and the current SDK's Lua parser rejects the
invalid numbers too. An SDK still on the fork sends an invalid `initial_stream_id`,
`max_frame_size` or `initial_conn_window_size` when a flow sets one. Each row has a test. The
CHANGELOG's **Breaking** section for 0.1.0 says which rows can make a working request fail.

| Input | Fork | This crate |
|---|---|---|
| Stream id with the top bit set (2³¹ or more) | accepted, and sent with that bit set: a dependency turns exclusive, a PRIORITY frame gets its reserved bit set | rejected, since wreq's `StreamId::from` panics on it |
| `initial_stream_id` that is even (including 0), or 2³¹ or more | accepted; HTTP/1.1 requests work, and every HTTP/2 connection made with it panics in the handshake | rejected |
| `max_frame_size` outside 16,384 to 16,777,215 | accepted; HTTP/1.1 requests work, and every HTTP/2 connection made with it panics in the handshake | rejected |
| `initial_conn_window_size` above 2³¹ − 1 | accepted; HTTP/1.1 requests work, and every HTTP/2 connection made with it panics in the handshake | rejected |
| `max_send_buffer_size` above `u32::MAX` | accepted; HTTP/1.1 requests work, and every HTTP/2 connection made with it panics in the handshake | rejected |
| `keep_alive_interval` or `keep_alive_timeout` above one day | accepted; wreq adds them to an `Instant` when it schedules a ping, which panics for values near `Duration::MAX` | rejected |
| `h1_max_buf_size` below 8192 | accepted; every HTTP/1 connection made with it then panics as it starts | rejected |
| Hand-written pseudo order, settings order or priorities (e.g. `{"ids": ["PATH"]}`) | kept as sent, so missing pseudo-headers and settings are left out of the request | kept as sent in the config. Converting to wreq completes and dedupes the list through wreq's builders, the only way to build these types outside wreq |
| The same JSON key twice in one object | at the top level and in `tls_options`: the last value wins, but `headers` and `orig_headers` merge both lists. Elsewhere: rejected | rejected everywhere |
| A key it does not know at the top level (`{"not_a_real_key": 1}`) | rejected | ignored |
| Whole config as a JSON array | rejected | accepted |
| `\u` escapes in `certificate_compression_algorithms` or `extension_permutation` | rejected | accepted |
| More than 24,576 distinct header names (the most an `http::HeaderMap` holds) | panics | returns an error |

## Configs that read fine but fail on every request

These configs are valid to read, like they are for the fork, but wreq returns an error for every
request made with them. It is an ordinary error, not a panic. Each was observed with wreq against
a local HTTP/1 or HTTP/2 server.

| Config | Why the request fails |
|---|---|
| `priorities` set, and `initial_stream_id` (1 if unset) not above the largest priority `stream_id` | The first request would reuse a stream id the PRIORITY frames already named, so http2 returns `OverflowedStreamId`. An empty `priorities` list counts as a largest id of 0, so it also fails with the default id of 1. With priorities on streams 3, 5 and 7, an `initial_stream_id` of 9 works. |
| `initial_window_size` above 2³¹ − 1 | It is above the largest HTTP/2 window (RFC 9113, section 6.9.2). 2³¹ − 1 works. |
| `header_table_size` of 2³¹ − 1 or more | The request fails. 16,777,216 works; values in between were not tried. |
| `max_header_list_size` of 0 or 1 | The response is rejected. |
| `h1_max_headers` of 0, 1 or `usize::MAX` | The response is rejected. |
| `h1_read_buf_exact_size` of 0, 1 or `usize::MAX` | The response can't be read. |

## Building

BoringSSL is built from source, so `cmake` must be on `PATH`. `rust-toolchain.toml` pins Rust
1.98.1, so pedantic clippy gives the same answer locally and in CI.

CI (`.github/workflows/ci.yml`) runs on every pull request to `main` and every push to it:
`cargo fmt --check`, `cargo clippy`, `cargo test`, and the golden-case check above.

```sh
cargo test
cargo clippy --all-targets
```
