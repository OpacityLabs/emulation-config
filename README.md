# emulation-config

Plain-data emulation configs for [wreq](https://github.com/OpacityLabs/wreq): default headers,
header order, and TLS, HTTP/1 and HTTP/2 options. It ships the browser presets
published by the wreq project as built-in profiles, and every field can be changed.

The JSON format is the one the SDK sends to the TEE. This crate reads and writes it byte for byte
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

let client = wreq::Client::builder()
    .emulation(wreq::Emulation::from(&config))
    .build()?;
```

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

`TlsConfig`, `Http1Config` and `Http2Config` also convert on their own into `TlsOptions`,
`Http1Options` and `Http2Options`. Conversion goes one way, from config to wreq.

`TlsConfig::default()`, `Http1Config::default()` and `Http2Config::default()` equal wreq's own
defaults. `PseudoOrder::new`, `SettingsOrder::new`, `Priorities::new` and
`ExperimentalSettings::new` clean up their input the same way wreq's builders do. So a config
built in code produces the same JSON the wreq builders produce. The SDK's Lua
parser builds configs this way.

## Versions and wreq

This crate has its own version. It pins wreq in `Cargo.toml` (and the exact commit in
`Cargo.lock`):

| emulation-config | wreq |
|---|---|
| 0.1.x | `OpacityLabs/wreq` `v6.0.0-rc.28.opacity.1`: no serde, http2 0.5.11 from crates.io |

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
- `experimental_settings` cannot carry anything. The format only names standard settings, and
  wreq drops standard ids from this list, so the SDK always sends it empty.

## How the format is checked

The reference is the fork's serde (`OpacityLabs/wreq` tag `v6.0.0-rc.28`), recorded in
`tests/golden/`. No expected output is written by hand, and the tests never call the fork's
serde, so they keep working once that code is removed from wreq.

- `tests/golden/wire_cases.json`: 51 inputs (edge cases, rejected inputs, every enum value) with
  what the fork wrote back or whether it rejected them, plus `Debug` of the wreq value each one
  must convert to.
- `tests/golden/profiles_wreq_debug.json`: `Debug` of the wreq value each profile must convert
  to.

A diff under `profiles/` or `tests/golden/` means a profile's fingerprint or the wire format
changed, so it comes with an entry in [CHANGELOG.md](CHANGELOG.md). Recorded cases are never
edited or removed, only added: CI fails if a case's `input` or `fork_output` differs from the
base branch (`.github/scripts/goldens_only_grow.py`).

The tests check that:
- every profile reproduces byte for byte through this crate's serde and `Profile::config()`, and
  converts to the recorded wreq value;
- every recorded case gives the fork's output and wreq value, or is rejected like the fork
  rejected it;
- defaults and the builder-style constructors match wreq's own;
- past the most distinct header names an `http::HeaderMap` holds, reading returns an error
  rather than panicking;
- the checked number types (`StreamId`, `InitialStreamId`, `MaxFrameSize`, `ConnWindowSize`,
  `MaxSendBufferSize`, `MaxBufSize`) reject exactly the values wreq and http2 panic on. The test
  runs values on each side of every limit through their real code, so an upgrade that changes a
  limit makes it fail;
- `patched()` matches making the same change in code, and the merge logic passes every example
  in RFC 7386, Appendix A.

## Known differences from the fork

No SDK flow produces any of these, and the SDK's Lua parser rejects the invalid
numbers too. Each one has a test.

| Input | Fork | This crate |
|---|---|---|
| Stream id with the top bit set (2³¹ or more) | accepted, and sent with that bit set: a dependency turns exclusive, a PRIORITY frame gets its reserved bit set | rejected, since wreq's `StreamId::from` panics on it |
| `initial_stream_id` that is even (including 0), or 2³¹ or more | accepted; every HTTP/2 connection made with it then panics in the handshake | rejected |
| `max_frame_size` outside 16,384 to 16,777,215 | accepted; every HTTP/2 connection made with it then panics in the handshake | rejected |
| `initial_conn_window_size` above 2³¹ − 1 | accepted; every HTTP/2 connection made with it then panics in the handshake | rejected |
| `max_send_buffer_size` above `u32::MAX` | accepted; every HTTP/2 connection made with it then panics in the handshake | rejected |
| `h1_max_buf_size` below 8192 | accepted; every HTTP/1 connection made with it then panics as it starts | rejected |
| Hand-written pseudo order, settings order or priorities (e.g. `{"ids": ["PATH"]}`) | kept as sent, so missing pseudo-headers and settings are left out of the request | kept as sent in the config. Converting to wreq completes and dedupes the list through wreq's builders, the only way to build these types outside wreq |
| The same JSON key twice in one object | at the top level and in `tls_options`: the last value wins, but `headers` and `orig_headers` merge both lists. Elsewhere: rejected | rejected everywhere |
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
