# Changelog

All notable changes to this crate. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), with an extra **Profiles** section, and
versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Every release states:
- **wreq**: the wreq tag it pins. A project using this release must use the same wreq.
- **Wire format**: whether the JSON the SDK sends to the TEE changed. If it did, deploy the TEE
  before any SDK that writes the new format.
- **API**: whether code that uses the crate has to change.
- **Profiles**: which built-in profiles were added, changed or removed. A changed profile sends a
  different fingerprint.

A **Breaking** section lists each change that makes a config, a request or code that worked
before fail or act differently, and who it affects.

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
  a keep-alive ping, which panics for values near `Duration::MAX`, so a TEE reading such a
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

Compared with the fork's serde, which the TEE used to read the config with:
- These numbers are now rejected, and with them the whole config:
  - an `initial_stream_id` that is even (including 0), or 2³¹ or more;
  - a `max_frame_size` outside 16,384 to 16,777,215;
  - an `initial_conn_window_size` above 2³¹ − 1;
  - a `max_send_buffer_size` above `u32::MAX`.

  The fork accepted them. HTTP/1.1 requests then worked, and HTTP/2 connections panicked. This
  was observed with wreq `v6.0.0-rc.28` and a local HTTP/1.1 server. An SDK that uses the fork
  sends the first three when a flow sets them, so after the TEE moves to this crate, such a flow
  fails on HTTP/1.1 sites too. No SDK flow sets them.
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
