//! The checked number types must reject exactly the values wreq's HTTP code panics on.
//!
//! Each candidate is run through the real code, so the rules are never restated by hand here:
//! - `StreamId`: wreq's `StreamId::from`.
//! - `InitialStreamId`, `MaxFrameSize` and `MaxSendBufferSize`: http2's client `Builder`.
//! - `ConnWindowSize`: an http2 client handshake over an in-memory pipe. The window is checked
//!   there, not in the builder.
//! - `MaxBufSize`: the `Http1Options` builder's `max_buf_size`. It checks the same minimum as an
//!   HTTP/1 connection does when it starts.
//! - `KeepAliveDuration`: `Instant + Duration`, which wreq's keep-alive code uses to schedule
//!   pings. Its limit is a round day, not the exact overflow point, so this only checks that the
//!   overflow is real and that the limit sits below it.
//!
//! If a wreq or http2 upgrade changes what panics, this test says so. It is the only test in its
//! binary because it swaps the global panic hook to keep the expected panics out of the output.

use std::{
    fmt::Debug,
    panic::{self, RefUnwindSafe, UnwindSafe},
    time::{Duration, Instant},
};

use emulation_config::{
    http1::MaxBufSize,
    http2::{
        ConnWindowSize, InitialStreamId, KeepAliveDuration, MaxFrameSize, MaxSendBufferSize,
        StreamId,
    },
};

/// Values on each side of every boundary: zero, odd and even, and the reserved top bit.
const STREAM_IDS: [u32; 12] = [
    0,
    1,
    2,
    3,
    4,
    15,
    (1 << 31) - 2,
    (1 << 31) - 1,
    1 << 31,
    (1 << 31) + 1,
    u32::MAX - 1,
    u32::MAX,
];

/// Values on each side of the 16 KiB minimum and the 16 MiB maximum.
const FRAME_SIZES: [u32; 9] = [
    0,
    1,
    16_383,
    16_384,
    16_385,
    16_777_214,
    16_777_215,
    16_777_216,
    u32::MAX,
];

/// Values on each side of the 2^31 - 1 maximum.
const CONN_WINDOW_SIZES: [u32; 6] = [0, 65_535, (1 << 31) - 2, (1 << 31) - 1, 1 << 31, u32::MAX];

/// Values on each side of `u32::MAX`.
const SEND_BUFFER_SIZES: [usize; 6] = [
    0,
    1,
    u32::MAX as usize - 1,
    u32::MAX as usize,
    u32::MAX as usize + 1,
    usize::MAX,
];

/// Values on each side of the 8 KiB minimum, plus wreq's default (about 400 KiB).
const BUFFER_SIZES: [usize; 7] = [0, 1, 8191, 8192, 8193, 8192 + 4096 * 100, usize::MAX];

/// Records each value where our type and wreq disagree.
fn compare<T, F>(
    mismatches: &mut Vec<String>,
    name: &str,
    values: &[T],
    run_in_wreq: F,
    rejected: impl Fn(T) -> bool,
) where
    T: Copy + Debug + UnwindSafe,
    F: Fn(T) + RefUnwindSafe,
{
    let run_in_wreq = &run_in_wreq;
    for &value in values {
        let panics = panic::catch_unwind(move || run_in_wreq(value)).is_err();
        if panics != rejected(value) {
            mismatches.push(format!(
                "{name} {value:?}: wreq panics: {panics}, we reject: {}",
                rejected(value)
            ));
        }
    }
}

/// Whether adding this duration to the current instant panics, as wreq's `proto/h2/ping.rs`
/// does with the keep-alive interval and timeout.
fn instant_plus(duration: Duration) -> bool {
    panic::catch_unwind(move || {
        let _ = Instant::now() + duration;
    })
    .is_err()
}

/// Starts an http2 client connection with this connection window.
fn handshake_with_conn_window(size: u32) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        // The server end stays alive so writing the connection preface succeeds.
        let (client, _server) = tokio::io::duplex(1 << 16);
        let _ = wreq_http2::client::Builder::new()
            .initial_connection_window_size(size)
            .handshake::<_, bytes::Bytes>(client)
            .await;
    });
}

#[test]
fn checked_numbers_reject_exactly_what_wreq_panics_on() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut mismatches = Vec::new();
    compare(
        &mut mismatches,
        "StreamId",
        &STREAM_IDS,
        |id| {
            let _ = wreq::http2::StreamId::from(id);
        },
        |id| StreamId::new(id).is_err(),
    );
    compare(
        &mut mismatches,
        "InitialStreamId",
        &STREAM_IDS,
        |id| {
            wreq_http2::client::Builder::new().initial_stream_id(id);
        },
        |id| InitialStreamId::new(id).is_err(),
    );
    compare(
        &mut mismatches,
        "MaxFrameSize",
        &FRAME_SIZES,
        |size| {
            wreq_http2::client::Builder::new().max_frame_size(size);
        },
        |size| MaxFrameSize::new(size).is_err(),
    );
    compare(
        &mut mismatches,
        "ConnWindowSize",
        &CONN_WINDOW_SIZES,
        handshake_with_conn_window,
        |size| ConnWindowSize::new(size).is_err(),
    );
    compare(
        &mut mismatches,
        "MaxSendBufferSize",
        &SEND_BUFFER_SIZES,
        |size| {
            wreq_http2::client::Builder::new().max_send_buffer_size(size);
        },
        |size| MaxSendBufferSize::new(size).is_err(),
    );
    compare(
        &mut mismatches,
        "MaxBufSize",
        &BUFFER_SIZES,
        |size| {
            let _ = wreq::http1::Http1Options::builder().max_buf_size(size);
        },
        |size| MaxBufSize::new(size).is_err(),
    );
    if !instant_plus(Duration::MAX) {
        mismatches.push("KeepAliveDuration: Instant + Duration::MAX does not panic".to_owned());
    }
    if instant_plus(KeepAliveDuration::MAX.get()) {
        mismatches.push("KeepAliveDuration: Instant + KeepAliveDuration::MAX panics".to_owned());
    }
    panic::set_hook(default_hook);
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}
