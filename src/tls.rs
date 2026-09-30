//! TLS options: what goes into the `ClientHello`.

use std::borrow::Cow;

use serde::{Deserialize, Deserializer, Serialize};
use wreq::tls as wreq_tls;

use crate::string_enum::string_enum;

string_enum! {
    /// A protocol offered through ALPN.
    pub enum AlpnProtocol("ALPN protocol") {
        Http1 => "http/1.1",
        Http2 => "h2",
        Http3 => "h3",
    }
}

string_enum! {
    /// A protocol that ALPS settings are sent for.
    pub enum AlpsProtocol("ALPS protocol") {
        Http1 => "http/1.1",
        Http2 => "h2",
        Http3 => "h3",
    }
}

string_enum! {
    /// A TLS protocol version.
    pub enum TlsVersion("TLS version") {
        Tls10 => "TLS_1_0",
        Tls11 => "TLS_1_1",
        Tls12 => "TLS_1_2",
        Tls13 => "TLS_1_3",
    }
}

string_enum! {
    /// A certificate compression algorithm (RFC 8879).
    pub enum CertificateCompressionAlgorithm("certificate compression algorithm") {
        Zlib => "zlib",
        Brotli => "brotli",
        Zstd => "zstd",
    }
}

string_enum! {
    /// A `ClientHello` extension, used to set the extension order.
    pub enum ExtensionType("TLS extension") {
        ServerName => "SERVER_NAME",
        StatusRequest => "STATUS_REQUEST",
        EcPointFormats => "EC_POINT_FORMATS",
        SignatureAlgorithms => "SIGNATURE_ALGORITHMS",
        Srtp => "SRTP",
        ApplicationLayerProtocolNegotiation => "APPLICATION_LAYER_PROTOCOL_NEGOTIATION",
        Padding => "PADDING",
        ExtendedMasterSecret => "EXTENDED_MASTER_SECRET",
        QuicTransportParametersLegacy => "QUIC_TRANSPORT_PARAMETERS_LEGACY",
        QuicTransportParametersStandard => "QUIC_TRANSPORT_PARAMETERS_STANDARD",
        CertCompression => "CERT_COMPRESSION",
        SessionTicket => "SESSION_TICKET",
        SupportedGroups => "SUPPORTED_GROUPS",
        PreSharedKey => "PRE_SHARED_KEY",
        EarlyData => "EARLY_DATA",
        SupportedVersions => "SUPPORTED_VERSIONS",
        Cookie => "COOKIE",
        PskKeyExchangeModes => "PSK_KEY_EXCHANGE_MODES",
        CertificateAuthorities => "CERTIFICATE_AUTHORITIES",
        SignatureAlgorithmsCert => "SIGNATURE_ALGORITHMS_CERT",
        KeyShare => "KEY_SHARE",
        Renegotiate => "RENEGOTIATE",
        DelegatedCredential => "DELEGATED_CREDENTIAL",
        ApplicationSettings => "APPLICATION_SETTINGS",
        ApplicationSettingsNew => "APPLICATION_SETTINGS_NEW",
        EncryptedClientHello => "ENCRYPTED_CLIENT_HELLO",
        CertificateTimestamp => "CERTIFICATE_TIMESTAMP",
        NextProtoNeg => "NEXT_PROTO_NEG",
        ChannelId => "CHANNEL_ID",
        RecordSizeLimit => "RECORD_SIZE_LIMIT",
    }
}

/// TLS options, mirroring `wreq::tls::TlsOptions`.
///
/// Two kinds of defaults apply:
/// - [`TlsConfig::default()`] matches `TlsOptions::default()`, so a config built in code starts
///   where wreq's builder starts.
/// - When reading JSON, a missing key (or a `null` flag) gets the value the fork's reader gives
///   it. These match wreq's defaults except for one: a missing `alpn_protocols` means no ALPN
///   at all, not `["h2", "http/1.1"]`.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag maps to one field of the wire format"
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlsConfig {
    pub alpn_protocols: Option<Vec<AlpnProtocol>>,
    pub alps_protocols: Option<Vec<AlpsProtocol>>,
    #[serde(default, deserialize_with = "null_as_false")]
    pub alps_use_new_codepoint: bool,
    #[serde(default = "default_true", deserialize_with = "null_as_true")]
    pub session_ticket: bool,
    pub min_tls_version: Option<TlsVersion>,
    pub max_tls_version: Option<TlsVersion>,
    #[serde(default, deserialize_with = "null_as_false")]
    pub pre_shared_key: bool,
    #[serde(default, deserialize_with = "null_as_false")]
    pub enable_ech_grease: bool,
    pub permute_extensions: Option<bool>,
    pub grease_enabled: Option<bool>,
    #[serde(default, deserialize_with = "null_as_false")]
    pub enable_ocsp_stapling: bool,
    #[serde(default, deserialize_with = "null_as_false")]
    pub enable_signed_cert_timestamps: bool,
    pub record_size_limit: Option<u16>,
    #[serde(default, deserialize_with = "null_as_false")]
    pub psk_skip_session_ticket: bool,
    pub key_shares_limit: Option<u8>,
    #[serde(default = "default_true", deserialize_with = "null_as_true")]
    pub psk_dhe_ke: bool,
    #[serde(default = "default_true", deserialize_with = "null_as_true")]
    pub renegotiation: bool,
    pub delegated_credentials: Option<String>,
    pub curves_list: Option<String>,
    pub cipher_list: Option<String>,
    pub sigalgs_list: Option<String>,
    pub certificate_compression_algorithms: Option<Vec<CertificateCompressionAlgorithm>>,
    pub extension_permutation: Option<Vec<ExtensionType>>,
    pub aes_hw_override: Option<bool>,
    /// Keep the TLS 1.3 ciphers in the order `cipher_list` gives them.
    ///
    /// The wire name is `prefer_chacha20`, kept for compatibility with existing payloads.
    #[serde(rename = "prefer_chacha20")]
    pub preserve_tls13_cipher_list: Option<bool>,
    #[serde(default, deserialize_with = "null_as_false")]
    pub random_aes_hw_override: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            alpn_protocols: Some(vec![AlpnProtocol::Http2, AlpnProtocol::Http1]),
            alps_protocols: None,
            alps_use_new_codepoint: false,
            session_ticket: true,
            min_tls_version: None,
            max_tls_version: None,
            pre_shared_key: false,
            enable_ech_grease: false,
            permute_extensions: None,
            grease_enabled: None,
            enable_ocsp_stapling: false,
            enable_signed_cert_timestamps: false,
            record_size_limit: None,
            psk_skip_session_ticket: false,
            key_shares_limit: None,
            psk_dhe_ke: true,
            renegotiation: true,
            delegated_credentials: None,
            curves_list: None,
            cipher_list: None,
            sigalgs_list: None,
            certificate_compression_algorithms: None,
            extension_permutation: None,
            aes_hw_override: None,
            preserve_tls13_cipher_list: None,
            random_aes_hw_override: false,
        }
    }
}

fn default_true() -> bool {
    true
}

fn null_as_false<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(Option::<bool>::deserialize(deserializer)?.unwrap_or(false))
}

fn null_as_true<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(Option::<bool>::deserialize(deserializer)?.unwrap_or(true))
}

/// Converts a list of our values into the owned slice wreq stores.
fn wreq_list<T, U>(list: Option<&[T]>) -> Option<Cow<'static, [U]>>
where
    T: Copy,
    U: From<T> + Clone,
{
    list.map(|items| Cow::Owned(items.iter().copied().map(U::from).collect()))
}

fn wreq_str(value: Option<&String>) -> Option<Cow<'static, str>> {
    value.map(|value| Cow::Owned(value.clone()))
}

impl From<&TlsConfig> for wreq_tls::TlsOptions {
    fn from(config: &TlsConfig) -> Self {
        // Fields are set one by one because `TlsOptions` is `#[non_exhaustive]`.
        // The builder can't be used either: it has no way to set `alpn_protocols` to `None`.
        let mut options = Self::default();
        options.alpn_protocols = wreq_list(config.alpn_protocols.as_deref());
        options.alps_protocols = wreq_list(config.alps_protocols.as_deref());
        options.alps_use_new_codepoint = config.alps_use_new_codepoint;
        options.session_ticket = config.session_ticket;
        options.min_tls_version = config.min_tls_version.map(Into::into);
        options.max_tls_version = config.max_tls_version.map(Into::into);
        options.pre_shared_key = config.pre_shared_key;
        options.enable_ech_grease = config.enable_ech_grease;
        options.permute_extensions = config.permute_extensions;
        options.grease_enabled = config.grease_enabled;
        options.enable_ocsp_stapling = config.enable_ocsp_stapling;
        options.enable_signed_cert_timestamps = config.enable_signed_cert_timestamps;
        options.record_size_limit = config.record_size_limit;
        options.psk_skip_session_ticket = config.psk_skip_session_ticket;
        options.key_shares_limit = config.key_shares_limit;
        options.psk_dhe_ke = config.psk_dhe_ke;
        options.renegotiation = config.renegotiation;
        options.delegated_credentials = wreq_str(config.delegated_credentials.as_ref());
        options.curves_list = wreq_str(config.curves_list.as_ref());
        options.cipher_list = wreq_str(config.cipher_list.as_ref());
        options.sigalgs_list = wreq_str(config.sigalgs_list.as_ref());
        options.certificate_compression_algorithms =
            wreq_list(config.certificate_compression_algorithms.as_deref());
        options.extension_permutation = wreq_list(config.extension_permutation.as_deref());
        options.aes_hw_override = config.aes_hw_override;
        options.preserve_tls13_cipher_list = config.preserve_tls13_cipher_list;
        options.random_aes_hw_override = config.random_aes_hw_override;
        options
    }
}

impl From<AlpnProtocol> for wreq_tls::AlpnProtocol {
    fn from(protocol: AlpnProtocol) -> Self {
        match protocol {
            AlpnProtocol::Http1 => Self::HTTP1,
            AlpnProtocol::Http2 => Self::HTTP2,
            AlpnProtocol::Http3 => Self::HTTP3,
        }
    }
}

impl From<AlpsProtocol> for wreq_tls::AlpsProtocol {
    fn from(protocol: AlpsProtocol) -> Self {
        match protocol {
            AlpsProtocol::Http1 => Self::HTTP1,
            AlpsProtocol::Http2 => Self::HTTP2,
            AlpsProtocol::Http3 => Self::HTTP3,
        }
    }
}

impl From<TlsVersion> for wreq_tls::TlsVersion {
    fn from(version: TlsVersion) -> Self {
        match version {
            TlsVersion::Tls10 => Self::TLS_1_0,
            TlsVersion::Tls11 => Self::TLS_1_1,
            TlsVersion::Tls12 => Self::TLS_1_2,
            TlsVersion::Tls13 => Self::TLS_1_3,
        }
    }
}

impl From<CertificateCompressionAlgorithm> for wreq_tls::CertificateCompressionAlgorithm {
    fn from(algorithm: CertificateCompressionAlgorithm) -> Self {
        match algorithm {
            CertificateCompressionAlgorithm::Zlib => Self::ZLIB,
            CertificateCompressionAlgorithm::Brotli => Self::BROTLI,
            CertificateCompressionAlgorithm::Zstd => Self::ZSTD,
        }
    }
}

impl From<ExtensionType> for wreq_tls::ExtensionType {
    fn from(extension: ExtensionType) -> Self {
        match extension {
            ExtensionType::ServerName => Self::SERVER_NAME,
            ExtensionType::StatusRequest => Self::STATUS_REQUEST,
            ExtensionType::EcPointFormats => Self::EC_POINT_FORMATS,
            ExtensionType::SignatureAlgorithms => Self::SIGNATURE_ALGORITHMS,
            ExtensionType::Srtp => Self::SRTP,
            ExtensionType::ApplicationLayerProtocolNegotiation => {
                Self::APPLICATION_LAYER_PROTOCOL_NEGOTIATION
            }
            ExtensionType::Padding => Self::PADDING,
            ExtensionType::ExtendedMasterSecret => Self::EXTENDED_MASTER_SECRET,
            ExtensionType::QuicTransportParametersLegacy => Self::QUIC_TRANSPORT_PARAMETERS_LEGACY,
            ExtensionType::QuicTransportParametersStandard => {
                Self::QUIC_TRANSPORT_PARAMETERS_STANDARD
            }
            ExtensionType::CertCompression => Self::CERT_COMPRESSION,
            ExtensionType::SessionTicket => Self::SESSION_TICKET,
            ExtensionType::SupportedGroups => Self::SUPPORTED_GROUPS,
            ExtensionType::PreSharedKey => Self::PRE_SHARED_KEY,
            ExtensionType::EarlyData => Self::EARLY_DATA,
            ExtensionType::SupportedVersions => Self::SUPPORTED_VERSIONS,
            ExtensionType::Cookie => Self::COOKIE,
            ExtensionType::PskKeyExchangeModes => Self::PSK_KEY_EXCHANGE_MODES,
            ExtensionType::CertificateAuthorities => Self::CERTIFICATE_AUTHORITIES,
            ExtensionType::SignatureAlgorithmsCert => Self::SIGNATURE_ALGORITHMS_CERT,
            ExtensionType::KeyShare => Self::KEY_SHARE,
            ExtensionType::Renegotiate => Self::RENEGOTIATE,
            ExtensionType::DelegatedCredential => Self::DELEGATED_CREDENTIAL,
            ExtensionType::ApplicationSettings => Self::APPLICATION_SETTINGS,
            ExtensionType::ApplicationSettingsNew => Self::APPLICATION_SETTINGS_NEW,
            ExtensionType::EncryptedClientHello => Self::ENCRYPTED_CLIENT_HELLO,
            ExtensionType::CertificateTimestamp => Self::CERTIFICATE_TIMESTAMP,
            ExtensionType::NextProtoNeg => Self::NEXT_PROTO_NEG,
            ExtensionType::ChannelId => Self::CHANNEL_ID,
            ExtensionType::RecordSizeLimit => Self::RECORD_SIZE_LIMIT,
        }
    }
}
