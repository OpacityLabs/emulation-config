# Changelog

All notable changes to this crate. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), with an extra **Profiles** section, and
versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Every release states:
- **wreq**: the wreq tag it pins. A project using this release must use the same wreq.
- **Wire format**: whether the JSON the SDK sends to the TEE changed. If it did, deploy the TEE
  before any SDK that writes the new format.
- **Profiles**: which built-in profiles were added, changed or removed. A changed profile sends a
  different fingerprint.

## [0.1.0] - 2026-09-30

wreq: `OpacityLabs/wreq` `v6.0.0-rc.28.opacity.1`
Wire format: the same bytes as the serde code in `OpacityLabs/wreq` `v6.0.0-rc.28`. The few
inputs read differently, none of which the SDK produces, are listed in the README.

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
