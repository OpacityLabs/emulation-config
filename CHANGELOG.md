# Changelog

All notable changes to this crate. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), with an extra **Profiles** section, and
versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Every release states:
- **wreq**: the wreq tag it pins. A project using this release must use the same wreq.
- **Wire format**: whether the JSON the SDK writes changed. If it did, update every reader before
  any SDK that writes the new format.
- **API**: whether code that uses the crate has to change.
- **Profiles**: which built-in profiles were added, changed or removed. A changed profile sends a
  different fingerprint.

A **Breaking** section lists each change that makes a config, a request or code that worked
before fail or act differently, and who it affects.

## [0.5.0] - 2026-11-10

wreq: `OpacityLabs/wreq` `v0.16.3-opacity.1`, the same as 0.4.0. wreq now needs its `cookies`
feature as well.
Wire format: one optional top-level key added, `cookies`. A config that keeps the default writes
no `cookies` key, so its JSON is unchanged. A reader on 0.4.x or older rejects a config that has
the key, because it rejects unknown top-level keys, so update every reader before any SDK or flow
sets it. A reader on 0.5.0 or later ignores keys it does not know, see Changed.
API: `EmulationConfig` has a new public field, see Breaking. Four items are new, see Added.
Profiles: the Chrome and Firefox profiles set `cookies`, see Profiles.

### Breaking
- `EmulationConfig` has a new public field, `cookies`. Code that builds an `EmulationConfig` with
  a struct literal and no `..EmulationConfig::default()` no longer compiles.

### Added
- `CookieConfig`: how a client lays out, orders and encodes the cookies from its cookie store,
  with `CookieLayout`, `CookieOverwrite` and the presets `CookieConfig::CHROME` and
  `CookieConfig::FIREFOX`. The default is one combined field and the RFC 6265 order, which is
  what the SDK sent before, with a fixed order. `wreq::Emulation::from(&config)` does not carry
  it: only a cookie store that reads it applies it.
- `CookieConfig::request_cookies`: turns the `name=value` pairs for a request into the wreq
  `Cookies` for its HTTP version. A `wreq::cookie::CookieStore` calls it from `cookies()`.
- `CookieOverwrite::renews_creation_time`: tells a cookie store whether a cookie that a response
  sets again moves to the end.
- `cookie_pairs`: splits a `Cookie` header value into its pairs.

### Changed
- Unknown keys are ignored in every object except a duration (`{"secs": .., "nanos": ..}`). A
  config with a key that a newer writer adds now reads, and the reader drops that key and its
  setting. An unknown top-level key was an error, as it is for the fork. An unknown value of a
  known key is still an error.

### Profiles
- `chrome_145_macos`, `chrome_145_windows`, `chrome_145_android`: `CookieConfig::CHROME`. One
  `cookie` field per cookie over HTTP/2, a cookie set again with a new value moves to the end,
  and no field is marked "never indexed".
- `firefox_147_macos`, `firefox_147_windows`, `firefox_android_135`: `CookieConfig::FIREFOX`.
  One `cookie` field per cookie over HTTP/2, a cookie set again keeps its place, and fields under
  20 bytes go out as HPACK "never indexed" literals.
- Both presets come from Chrome 156 and Firefox 157 on macOS, captured with Wireshark, and match
  each browser's source. The Android profiles use the same network code. The Safari profiles
  keep the default, because Safari is not measured.
- Known gap: http2 0.5.20 never adds a cookie field to the HPACK table, so every field that is
  not marked "never indexed" goes out as a literal without indexing on every request. Chrome and
  Firefox index those fields after the first request. See "Cookies" in the README.

## [0.4.0] - 2026-11-02

wreq: `OpacityLabs/wreq` `v0.16.3-opacity.1`, the fork of upstream `v0.16.3`
Wire format: one optional key added, `tls_options.key_shares`. A payload without it is unchanged.
A reader on 0.3.x or older ignores the key and uses `key_shares_limit` instead, so update readers
before any flow sets it.
API: `TlsConfig` has a new public field, see Breaking. Three items are new, see Added.

### Breaking
- The pinned wreq is now `v0.16.3-opacity.1`. A project that moves to this release must move its
  own wreq to the same tag, pinned as `version = "0.16.3-opacity.1", tag = "v0.16.3-opacity.1"`,
  or Cargo builds two copies of wreq and the project does not compile.
- `TlsConfig` has a new public field, `key_shares`. Code that builds a `TlsConfig` with a struct
  literal and no `..TlsConfig::default()` no longer compiles.

### Added
- `TlsConfig::key_shares`: the groups that get a key share, named as in `curves_list`
  (`tls::KeyShare`). It is wreq 0.16's own option and wins over `key_shares_limit`. When unset it
  is left out of the JSON.
- `EmulationConfig::move_cookie_slot_to_header_order`: moves a `cookie` entry out of `headers`
  into the header order and returns its value. The cookies wreq takes from a cookie store then go
  in that slot, and over HTTP/2 they go as one `cookie` field per cookie, as browsers send them.
- `OrigHeaders::contains`.

### Profiles
- None changed.

## [0.3.0] - 2026-11-02

wreq: `OpacityLabs/wreq` `v0.16.1-opacity.1`, the fork of upstream `v0.16.1`
Wire format: unchanged. Two fields convert to wreq differently, see Breaking.
API: `EmulationConfig` converts into `wreq::Emulation` by value as well as by reference; wreq's
`IntoEmulation` covers that, so `emulation(config)` still works. `wreq::EmulationFactory` no
longer exists. `Http2Config::experimental_settings` is deprecated.

### Breaking
- The pinned wreq is `v0.16.1-opacity.1`. A project that moves to this release must move its
  own wreq to the same tag, enable wreq's `tokio-rt` feature (wreq 0.16 does not build without a
  runtime feature), and pin `http2` 0.5.20 where it pins the http2 crate. The crate version at
  that tag is a pre-release, which Cargo only matches when the requirement names it, so pin it as
  `version = "0.16.1-opacity.1", tag = "v0.16.1-opacity.1"`.
- `key_shares_limit` now derives wreq's `key_shares`: the first N groups of `curves_list`.
  Upstream replaced the limit with an explicit list when it moved from boring2 to btls, and
  this is the mapping its own Firefox presets use. The three Firefox profiles send the same key
  shares as before. A config with a limit and no `curves_list` gets BoringSSL's default shares.
- `experimental_settings` is dead: wreq removed the option (`0x676e67/wreq#1109`). It is still
  read and written, and the conversion drops it. No flow or profile sets it, and the format
  cannot name a setting wreq would have kept, so nothing on the wire changes.
- The TLS binding is btls instead of boring2 and the HTTP/2 crate is 0.5.20 instead of 0.5.11.
  The JA4 and Akamai fingerprints of every profile should be compared against a capture before
  relying on this release.

### Added
- The zlib, brotli and zstd certificate compressors, which wreq 0.16 no longer ships. The
  conversion registers them for the algorithms a config names, so the `compress_certificate`
  extension stays as it was.

### Changed
- The recorded wreq values moved from `tests/golden/profiles_wreq_debug.json` and the
  `wreq_debug` fields of `wire_cases.json` to `tests/golden/wreq_debug.json`, a snapshot of
  this crate's own conversion that `UPDATE_WREQ_DEBUG=1 cargo test` rewrites.
  `wire_cases.json` keeps only the wire contract: `input` and `fork_output`.

### Profiles
- None changed.

## [0.2.0] - 2026-10-19

wreq: `OpacityLabs/wreq` `v6.0.0-rc.28.opacity.2`
Wire format: unchanged.
API: unchanged. Code that uses this crate builds and works the same.

### Breaking
- The pinned wreq is now `v6.0.0-rc.28.opacity.2`. A project that moves to this release must move
  its own wreq to the same tag, or it builds two copies of wreq and does not compile. In that
  wreq, `ClientBuilder::socket(i32)` is replaced by `ClientBuilder::tcp_stream(TcpStream)`, so
  code that calls `socket` must change.

### Profiles
- None changed.

## [0.1.2] - 2026-10-09

wreq: `OpacityLabs/wreq` `v6.0.0-rc.28.opacity.1`
Wire format: unchanged.
API: `Http2Config::keep_alive_interval` and `keep_alive_timeout` hold a `KeepAliveDuration`
instead of a `Duration`. Code that writes them wraps the value with `KeepAliveDuration::new`, and
code that reads them calls `get()`.

### Breaking
- A `keep_alive_interval` or `keep_alive_timeout` above one day is now rejected, and with it
  the whole config. The fork accepted any value. wreq adds both to an `Instant` when it schedules
  a keep-alive ping, which panics for values near `Duration::MAX`, so a client built from such a
  config would panic on the first HTTP/2 connection. No SDK flow and no profile sets either
  field.

### Added
- `http2::KeepAliveDuration`, with `KeepAliveDuration::MAX` (one day).

### Profiles
- None changed.

## [0.1.1] - 2026-10-02

wreq: `OpacityLabs/wreq` `v6.0.0-rc.28.opacity.1`
Wire format: unchanged.
API: additive. Code written for 0.1.0 builds and works the same. Code that calls the new methods
needs 0.1.1.

### Added
- `Http2Config::set_adaptive_window`, `set_initial_window_size` and
  `set_initial_conn_window_size`, which apply wreq's builder rules for the HTTP/2 windows.
- `wreq::EmulationFactory` for `EmulationConfig`, so wreq's `emulation(...)` methods take a
  config directly.

### Profiles
- None changed.

## [0.1.0] - 2026-09-30

wreq: `OpacityLabs/wreq` `v6.0.0-rc.28.opacity.1`
Wire format: writes the same bytes as the serde code in `OpacityLabs/wreq` `v6.0.0-rc.28`. Reads
the same, except the inputs under **Breaking**. The README's "Known differences from the fork"
lists every input read differently.
API: first release.

### Breaking

Compared with the fork's serde:
- These numbers are now rejected, and with them the whole config:
  - an `initial_stream_id` that is even (including 0), or 2³¹ or more;
  - a `max_frame_size` outside 16,384 to 16,777,215;
  - an `initial_conn_window_size` above 2³¹ − 1;
  - a `max_send_buffer_size` above `u32::MAX`.

  The fork accepted them. HTTP/1.1 requests then worked, and HTTP/2 connections panicked. This
  was observed with wreq `v6.0.0-rc.28` and a local HTTP/1.1 server. An SDK that uses the fork
  sends the first three when a flow sets them, so once its configs are read with this crate, such
  a flow fails on HTTP/1.1 sites too. No SDK flow sets them.
- A stream id with the top bit set (2³¹ or more) is now rejected. The fork sent it with that bit
  set. SDKs never send one: the Lua parser on the fork panics on it, and the current one rejects
  it.
- An `h1_max_buf_size` below 8192 is now rejected. SDKs never send HTTP/1 options.
- The same JSON key twice at the top level or in `tls_options` is now rejected. The fork used
  the last value, and merged both lists for `headers` and `orig_headers`. SDKs never send repeated
  keys.
- A hand-written pseudo-header order, settings order or priority list that wreq's builders would
  complete is now completed and deduplicated when it converts to wreq. The fork sent it as it was,
  so the HTTP/2 frames differ. SDKs always send completed lists.

### Added
- `EmulationConfig`, `TlsConfig`, `Http1Config` and `Http2Config`: the emulation config as plain
  data, with serde for the wire format.
- Conversion into wreq types: `wreq::Emulation::from(&config)`, and the same for each part.
- `EmulationConfig::patched()` for JSON merge patches (RFC 7386).
- `PseudoOrder::new`, `SettingsOrder::new`, `Priorities::new` and `ExperimentalSettings::new`,
  which clean up their input the way wreq's builders do.
- `StreamId`, `InitialStreamId`, `MaxFrameSize`, `ConnWindowSize`, `MaxSendBufferSize` and
  `MaxBufSize`: numbers that reject the values wreq's HTTP/1 and HTTP/2 code panics on.

### Profiles
- Added 9 profiles: the wreq-util 3.0.0-rc.10 browser presets. The README lists them.
