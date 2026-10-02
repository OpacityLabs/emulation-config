//! HTTP/2 options: SETTINGS, pseudo-header order, priorities.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use wreq::http2 as wreq_h2;

use crate::{checked_number::checked_number, string_enum::string_enum};

string_enum! {
    /// An HTTP/2 pseudo-header. Declared in wreq's default order.
    pub enum PseudoId("pseudo-header") {
        Method => "METHOD",
        Scheme => "SCHEME",
        Authority => "AUTHORITY",
        Path => "PATH",
        Protocol => "PROTOCOL",
        Status => "STATUS",
    }
}

string_enum! {
    /// A standard HTTP/2 setting. Declared in wreq's default order.
    pub enum SettingId("HTTP/2 setting") {
        HeaderTableSize => "HEADER_TABLE_SIZE",
        EnablePush => "ENABLE_PUSH",
        MaxConcurrentStreams => "MAX_CONCURRENT_STREAMS",
        InitialWindowSize => "INITIAL_WINDOW_SIZE",
        MaxFrameSize => "MAX_FRAME_SIZE",
        MaxHeaderListSize => "MAX_HEADER_LIST_SIZE",
        EnableConnectProtocol => "ENABLE_CONNECT_PROTOCOL",
        NoRfc7540Priorities => "NO_RFC7540_PRIORITIES",
    }
}

/// HTTP/2's default flow-control window (RFC 9113, section 6.9.2). wreq's builder switches both
/// window sizes back to it when `adaptive_window` is turned on.
pub const SPEC_WINDOW_SIZE: u32 = 65_535;

checked_number! {
    /// An HTTP/2 stream id.
    ///
    /// Values with the reserved top bit set are rejected. wreq's `StreamId` panics on them.
    pub struct StreamId(u32);

    /// A stream id with the reserved top bit set.
    pub struct InvalidStreamId: "stream id {0} has the reserved top bit set"
}

impl StreamId {
    /// Stream 0, the connection itself.
    pub const ZERO: Self = Self(0);

    const RESERVED_BIT: u32 = 1 << 31;

    const fn is_valid(id: u32) -> bool {
        id & Self::RESERVED_BIT == 0
    }
}

checked_number! {
    /// The id of the first stream the client opens.
    ///
    /// Only odd values below 2^31 are accepted: clients open odd-numbered streams, and the top bit
    /// is reserved. The HTTP/2 handshake panics on any other value.
    pub struct InitialStreamId(u32);

    /// An `initial_stream_id` that is even (including 0) or has the reserved top bit set.
    pub struct InvalidInitialStreamId: "initial stream id {0} must be odd and below 2^31"
}

impl InitialStreamId {
    const fn is_valid(id: u32) -> bool {
        id % 2 == 1 && StreamId::is_valid(id)
    }
}

checked_number! {
    /// The largest frame the client accepts (`SETTINGS_MAX_FRAME_SIZE`).
    ///
    /// HTTP/2 allows 16,384 to 16,777,215 bytes (RFC 9113, section 6.5.2). The HTTP/2 handshake
    /// panics on any other value.
    pub struct MaxFrameSize(u32);

    /// A `max_frame_size` outside the range HTTP/2 allows.
    pub struct InvalidMaxFrameSize:
        "max frame size {0} is outside {min}..={max}", min = MaxFrameSize::MIN, max = MaxFrameSize::MAX
}

impl MaxFrameSize {
    /// The smallest allowed value, which is also HTTP/2's default.
    pub const MIN: u32 = 1 << 14;
    /// The largest allowed value.
    pub const MAX: u32 = (1 << 24) - 1;

    const fn is_valid(size: u32) -> bool {
        size >= Self::MIN && size <= Self::MAX
    }
}

checked_number! {
    /// The flow-control window the client sets for the whole connection.
    ///
    /// HTTP/2 windows go up to 2^31 - 1 bytes (RFC 9113, section 6.9.1). The HTTP/2 handshake
    /// panics on larger values.
    pub struct ConnWindowSize(u32);

    /// An `initial_conn_window_size` above the largest HTTP/2 window.
    pub struct InvalidConnWindowSize:
        "connection window size {0} is above the maximum of {max}", max = ConnWindowSize::MAX
}

impl ConnWindowSize {
    /// The largest allowed value.
    pub const MAX: u32 = (1 << 31) - 1;
    /// [`SPEC_WINDOW_SIZE`], HTTP/2's default window.
    pub const SPEC_DEFAULT: Self = Self(SPEC_WINDOW_SIZE);

    const fn is_valid(size: u32) -> bool {
        size <= Self::MAX
    }
}

checked_number! {
    /// The most data buffered for sending per stream, in bytes.
    ///
    /// http2 stores it as a `u32` and panics on larger values when the connection starts.
    pub struct MaxSendBufferSize(usize);

    /// A `max_send_buffer_size` that does not fit in a `u32`.
    pub struct InvalidMaxSendBufferSize: "max send buffer size {0} does not fit in a u32"
}

impl MaxSendBufferSize {
    const fn is_valid(size: usize) -> bool {
        size <= u32::MAX as usize
    }
}

/// Order of pseudo-headers in outgoing HEADERS frames.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PseudoOrder {
    pub ids: Vec<PseudoId>,
}

impl PseudoOrder {
    /// Builds an order the way wreq's builder does: repeats are dropped, and unless all six
    /// pseudo-headers are listed, the missing ones are added at the end in default order.
    #[must_use]
    pub fn new(ids: impl IntoIterator<Item = PseudoId>) -> Self {
        let mut order = Vec::new();
        push_unique(&mut order, ids);
        if order.len() != PseudoId::ALL.len() {
            push_unique(&mut order, PseudoId::ALL.iter().copied());
        }
        Self { ids: order }
    }
}

/// Order of settings in the first SETTINGS frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsOrder {
    pub ids: Vec<SettingId>,
}

impl SettingsOrder {
    /// Builds an order the way wreq's builder does: repeats are dropped and every setting not
    /// listed is added at the end in default order.
    #[must_use]
    pub fn new(ids: impl IntoIterator<Item = SettingId>) -> Self {
        let mut order = Vec::new();
        push_unique(&mut order, ids);
        push_unique(&mut order, SettingId::ALL.iter().copied());
        Self { ids: order }
    }
}

fn push_unique<T: PartialEq>(list: &mut Vec<T>, items: impl IntoIterator<Item = T>) {
    for item in items {
        if !list.contains(&item) {
            list.push(item);
        }
    }
}

/// The stream a HEADERS or PRIORITY frame depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamDependency {
    pub dependency_id: StreamId,
    pub weight: u8,
    pub is_exclusive: bool,
}

/// A PRIORITY frame sent right after the connection starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Priority {
    pub stream_id: StreamId,
    pub dependency: StreamDependency,
}

/// The PRIORITY frames sent right after the connection starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Priorities {
    pub priorities: Vec<Priority>,
    /// The highest `stream_id` in `priorities`. Kept because it is part of the wire format.
    pub max_stream_id: StreamId,
}

impl Priorities {
    /// Builds the list the way wreq's builder does: frames for stream 0 and repeated stream ids
    /// are dropped, and `max_stream_id` is worked out from what is left.
    #[must_use]
    pub fn new(priorities: impl IntoIterator<Item = Priority>) -> Self {
        let mut kept: Vec<Priority> = Vec::new();
        let mut max_stream_id = StreamId::ZERO;
        for priority in priorities {
            let is_repeat = kept.iter().any(|p| p.stream_id == priority.stream_id);
            if priority.stream_id == StreamId::ZERO || is_repeat {
                continue;
            }
            max_stream_id = max_stream_id.max(priority.stream_id);
            kept.push(priority);
        }
        Self {
            priorities: kept,
            max_stream_id,
        }
    }
}

/// Extra SETTINGS entries.
///
/// wreq only keeps settings with non-standard ids, but the wire format can only name standard
/// ones. So this converts to an empty list in wreq, and the SDK always sends it empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentalSettings {
    pub settings: Vec<Setting>,
}

impl ExperimentalSettings {
    /// Builds the list the way wreq's builder does: only non-standard setting ids are kept.
    /// [`SettingId`] only names standard ones, so the result is always empty.
    #[must_use]
    pub fn new(_settings: impl IntoIterator<Item = Setting>) -> Self {
        Self {
            settings: Vec::new(),
        }
    }
}

/// One SETTINGS entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setting {
    pub id: SettingId,
    pub value: u32,
}

/// HTTP/2 options, mirroring `wreq::http2::Http2Options`.
///
/// Fields that are not `Option` are required in JSON, as they are for the fork. The `Default`
/// matches `Http2Options::default()`.
///
/// wreq's builder ties `adaptive_window` to the two window sizes. The `set_*` methods for those
/// fields apply the same rules. Writing the fields directly skips them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Http2Config {
    pub adaptive_window: bool,
    pub initial_stream_id: Option<InitialStreamId>,
    pub initial_conn_window_size: ConnWindowSize,
    pub initial_window_size: u32,
    pub initial_max_send_streams: usize,
    pub max_frame_size: Option<MaxFrameSize>,
    pub keep_alive_interval: Option<Duration>,
    pub keep_alive_timeout: Duration,
    pub keep_alive_while_idle: bool,
    pub max_concurrent_reset_streams: Option<usize>,
    pub max_send_buffer_size: MaxSendBufferSize,
    pub max_concurrent_streams: Option<u32>,
    pub max_header_list_size: Option<u32>,
    pub max_pending_accept_reset_streams: Option<usize>,
    pub enable_push: Option<bool>,
    pub header_table_size: Option<u32>,
    pub enable_connect_protocol: Option<bool>,
    pub no_rfc7540_priorities: Option<bool>,
    pub headers_pseudo_order: Option<PseudoOrder>,
    pub headers_stream_dependency: Option<StreamDependency>,
    pub experimental_settings: Option<ExperimentalSettings>,
    pub settings_order: Option<SettingsOrder>,
    pub priorities: Option<Priorities>,
}

impl Default for Http2Config {
    fn default() -> Self {
        Self {
            adaptive_window: false,
            initial_stream_id: None,
            initial_conn_window_size: ConnWindowSize(5 * 1024 * 1024),
            initial_window_size: 2 * 1024 * 1024,
            initial_max_send_streams: 100,
            max_frame_size: None,
            keep_alive_interval: None,
            keep_alive_timeout: Duration::from_secs(20),
            keep_alive_while_idle: false,
            max_concurrent_reset_streams: None,
            max_send_buffer_size: MaxSendBufferSize(1024 * 1024),
            max_concurrent_streams: None,
            max_header_list_size: None,
            max_pending_accept_reset_streams: None,
            enable_push: None,
            header_table_size: None,
            enable_connect_protocol: None,
            no_rfc7540_priorities: None,
            headers_pseudo_order: None,
            headers_stream_dependency: None,
            experimental_settings: None,
            settings_order: None,
            priorities: None,
        }
    }
}

impl Http2Config {
    /// Sets `adaptive_window` like wreq's builder does: turning it on also resets both window
    /// sizes to [`SPEC_WINDOW_SIZE`], where wreq's adaptive flow control starts.
    pub fn set_adaptive_window(&mut self, enabled: bool) {
        self.adaptive_window = enabled;
        if enabled {
            self.initial_window_size = SPEC_WINDOW_SIZE;
            self.initial_conn_window_size = ConnWindowSize::SPEC_DEFAULT;
        }
    }

    /// Sets the stream window like wreq's builder does: a fixed window turns `adaptive_window`
    /// off.
    pub fn set_initial_window_size(&mut self, size: u32) {
        self.adaptive_window = false;
        self.initial_window_size = size;
    }

    /// Sets the connection window like wreq's builder does: a fixed window turns
    /// `adaptive_window` off.
    pub fn set_initial_conn_window_size(&mut self, size: ConnWindowSize) {
        self.adaptive_window = false;
        self.initial_conn_window_size = size;
    }
}

impl From<&Http2Config> for wreq_h2::Http2Options {
    fn from(config: &Http2Config) -> Self {
        // Set field by field: the builder has side effects (setting a window size also turns off
        // `adaptive_window`), and the wire format stores the final values, not builder calls.
        let mut options = Self::default();
        options.adaptive_window = config.adaptive_window;
        options.initial_stream_id = config.initial_stream_id.map(InitialStreamId::get);
        options.initial_conn_window_size = config.initial_conn_window_size.get();
        options.initial_window_size = config.initial_window_size;
        options.initial_max_send_streams = config.initial_max_send_streams;
        options.max_frame_size = config.max_frame_size.map(MaxFrameSize::get);
        options.keep_alive_interval = config.keep_alive_interval;
        options.keep_alive_timeout = config.keep_alive_timeout;
        options.keep_alive_while_idle = config.keep_alive_while_idle;
        options.max_concurrent_reset_streams = config.max_concurrent_reset_streams;
        options.max_send_buffer_size = config.max_send_buffer_size.get();
        options.max_concurrent_streams = config.max_concurrent_streams;
        options.max_header_list_size = config.max_header_list_size;
        options.max_pending_accept_reset_streams = config.max_pending_accept_reset_streams;
        options.enable_push = config.enable_push;
        options.header_table_size = config.header_table_size;
        options.enable_connect_protocol = config.enable_connect_protocol;
        options.no_rfc7540_priorities = config.no_rfc7540_priorities;
        options.headers_pseudo_order = config.headers_pseudo_order.as_ref().map(Into::into);
        options.headers_stream_dependency = config.headers_stream_dependency.map(Into::into);
        options.experimental_settings = config.experimental_settings.as_ref().map(Into::into);
        options.settings_order = config.settings_order.as_ref().map(Into::into);
        options.priorities = config.priorities.as_ref().map(Into::into);
        options
    }
}

// The frame types below keep their fields private in wreq, so they can only be made through
// wreq's builders. For lists made by those same builders (everything the SDK sends), building
// again gives back the same list.

impl From<&PseudoOrder> for wreq_h2::PseudoOrder {
    fn from(order: &PseudoOrder) -> Self {
        Self::builder()
            .extend(order.ids.iter().copied().map(Into::into))
            .build()
    }
}

impl From<&SettingsOrder> for wreq_h2::SettingsOrder {
    fn from(order: &SettingsOrder) -> Self {
        Self::builder()
            .extend(order.ids.iter().copied().map(Into::into))
            .build()
    }
}

impl From<&Priorities> for wreq_h2::Priorities {
    fn from(priorities: &Priorities) -> Self {
        Self::builder()
            .extend(priorities.priorities.iter().copied().map(Into::into))
            .build()
    }
}

impl From<&ExperimentalSettings> for wreq_h2::ExperimentalSettings {
    fn from(settings: &ExperimentalSettings) -> Self {
        Self::builder()
            .extend(
                settings
                    .settings
                    .iter()
                    .map(|setting| wreq_h2::Setting::from_id(setting.id, setting.value)),
            )
            .build()
    }
}

impl From<Priority> for wreq_h2::Priority {
    fn from(priority: Priority) -> Self {
        Self::new(priority.stream_id.into(), priority.dependency.into())
    }
}

impl From<StreamDependency> for wreq_h2::StreamDependency {
    fn from(dependency: StreamDependency) -> Self {
        Self::new(
            dependency.dependency_id.into(),
            dependency.weight,
            dependency.is_exclusive,
        )
    }
}

impl From<StreamId> for wreq_h2::StreamId {
    fn from(id: StreamId) -> Self {
        // Cannot panic: `StreamId` never holds a value with the reserved bit set.
        Self::from(id.0)
    }
}

impl From<PseudoId> for wreq_h2::PseudoId {
    fn from(id: PseudoId) -> Self {
        match id {
            PseudoId::Method => Self::Method,
            PseudoId::Scheme => Self::Scheme,
            PseudoId::Authority => Self::Authority,
            PseudoId::Path => Self::Path,
            PseudoId::Protocol => Self::Protocol,
            PseudoId::Status => Self::Status,
        }
    }
}

impl From<SettingId> for wreq_h2::SettingId {
    fn from(id: SettingId) -> Self {
        match id {
            SettingId::HeaderTableSize => Self::HeaderTableSize,
            SettingId::EnablePush => Self::EnablePush,
            SettingId::MaxConcurrentStreams => Self::MaxConcurrentStreams,
            SettingId::InitialWindowSize => Self::InitialWindowSize,
            SettingId::MaxFrameSize => Self::MaxFrameSize,
            SettingId::MaxHeaderListSize => Self::MaxHeaderListSize,
            SettingId::EnableConnectProtocol => Self::EnableConnectProtocol,
            SettingId::NoRfc7540Priorities => Self::NoRfc7540Priorities,
        }
    }
}
