//! HTTP/1 options.

use serde::{Deserialize, Serialize};
use wreq::http1::Http1Options;

use crate::checked_number::checked_number;

checked_number! {
    /// The largest buffer an HTTP/1 connection uses, in bytes.
    ///
    /// Values below [`MaxBufSize::MIN`] are rejected. wreq panics on them when an HTTP/1
    /// connection starts.
    pub struct MaxBufSize(usize);

    /// A `h1_max_buf_size` below the smallest size wreq accepts.
    pub struct InvalidMaxBufSize:
        "max buffer size {0} is below the minimum of {min} bytes", min = MaxBufSize::MIN
}

impl MaxBufSize {
    /// The smallest size wreq accepts (8 KiB, its initial read buffer size).
    pub const MIN: usize = 8192;

    const fn is_valid(size: usize) -> bool {
        size >= Self::MIN
    }
}

/// HTTP/1 options, mirroring `wreq::http1::Http1Options`.
///
/// Every flag is required in JSON, as it is for the fork.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag maps to one field of the wire format"
)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Http1Config {
    pub h09_responses: bool,
    pub h1_writev: Option<bool>,
    pub h1_max_headers: Option<usize>,
    pub h1_read_buf_exact_size: Option<usize>,
    pub h1_max_buf_size: Option<MaxBufSize>,
    pub ignore_invalid_headers_in_responses: bool,
    pub allow_spaces_after_header_name_in_responses: bool,
    pub allow_obsolete_multiline_headers_in_responses: bool,
}

impl From<&Http1Config> for Http1Options {
    fn from(config: &Http1Config) -> Self {
        // Set field by field: in the builder, `read_buf_exact_size` and `max_buf_size` each unset
        // the other, while the wire format (and the fork) can carry both.
        let mut options = Self::default();
        options.h09_responses = config.h09_responses;
        options.h1_writev = config.h1_writev;
        options.h1_max_headers = config.h1_max_headers;
        options.h1_read_buf_exact_size = config.h1_read_buf_exact_size;
        options.h1_max_buf_size = config.h1_max_buf_size.map(MaxBufSize::get);
        options.ignore_invalid_headers_in_responses = config.ignore_invalid_headers_in_responses;
        options.allow_spaces_after_header_name_in_responses =
            config.allow_spaces_after_header_name_in_responses;
        options.allow_obsolete_multiline_headers_in_responses =
            config.allow_obsolete_multiline_headers_in_responses;
        options
    }
}
