//! Plain-data emulation configs for [wreq].
//!
//! An [`EmulationConfig`] describes everything a `wreq::Emulation` holds: default headers,
//! header order, and the TLS, HTTP/1 and HTTP/2 options. It reads and writes the same JSON as
//! the serde code inside the `OpacityLabs/wreq` fork (`v6.0.0-rc.28`), which is the format the
//! SDK sends to the TEE today.
//!
//! The types here are ours, so the wire format no longer depends on wreq's internals. Start from a
//! built-in [`Profile`] or from any JSON config, change what you need, and hand it to wreq:
//!
//! ```
//! use emulation_config::EmulationConfig;
//!
//! let config: EmulationConfig = serde_json::from_str(r#"{"transport":{}}"#)?;
//! let client = wreq::Client::builder().emulation(config).build();
//! # let _ = client;
//! # Ok::<(), serde_json::Error>(())
//! ```
//!
//! `wreq::Emulation::from(&config)` gives the wreq value itself, when you need it without a
//! builder.

mod checked_number;
mod emulation;
mod headers;
pub mod http1;
pub mod http2;
mod profiles;
mod string_enum;
pub mod tls;

pub use emulation::{EmulationConfig, TransportConfig};
pub use headers::{OrigHeaderName, OrigHeaders};
pub use http1::Http1Config;
pub use http2::Http2Config;
pub use profiles::Profile;
pub use string_enum::UnknownVariant;
pub use tls::TlsConfig;
