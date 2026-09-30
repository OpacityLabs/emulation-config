//! `EmulationConfig` must read and write exactly what the fork's serde does, and convert into
//! the same wreq values.
//!
//! The expected results live in `tests/golden/`, recorded from the fork's serde
//! (`OpacityLabs/wreq` tag `v6.0.0-rc.28`). No expected output here is written by hand. These
//! tests never call the fork's serde, so they keep working once that code is removed from wreq.

use std::collections::BTreeMap;

use emulation_config::{EmulationConfig, Profile};
use pretty_assertions::assert_eq;
use serde::Deserialize;
use wreq::header::{HeaderMap, HeaderName, HeaderValue};

/// One recorded input and what the fork did with it.
#[derive(Deserialize)]
struct Case {
    name: String,
    input: String,
    /// What the fork wrote back after reading `input`, or `None` if it rejected it.
    fork_output: Option<String>,
    /// `Debug` of the wreq value a correct conversion produces.
    wreq_debug: Option<String>,
}

/// Inputs we knowingly treat differently from the fork. Each one is explained in the README.
const WE_REJECT: [&str; 11] = [
    "stream_id_top_bit",
    "initial_stream_id_zero",
    "initial_stream_id_even",
    "initial_stream_id_top_bit",
    "max_frame_size_too_small",
    "max_frame_size_too_large",
    "conn_window_size_too_large",
    "max_send_buffer_size_too_large",
    "http1_small_buffer",
    "repeated_key",
    "repeated_headers_key",
];
const WE_ACCEPT: [&str; 3] = [
    "config_as_array",
    "escaped_certificate_compression",
    "escaped_extension_type",
];

fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!("golden/wire_cases.json")).unwrap()
}

fn profile_debug() -> BTreeMap<String, String> {
    serde_json::from_str(include_str!("golden/profiles_wreq_debug.json")).unwrap()
}

/// Reads `json` into our config and writes it back with our serde.
fn our_roundtrip(json: &str) -> serde_json::Result<String> {
    let config: EmulationConfig = serde_json::from_str(json)?;
    serde_json::to_string(&config)
}

/// Reads `json` into our config and returns `Debug` of the wreq value it converts to.
fn our_conversion(json: &str) -> serde_json::Result<String> {
    let config: EmulationConfig = serde_json::from_str(json)?;
    Ok(format!("{:?}", wreq::Emulation::from(&config)))
}

#[test]
fn profiles_match_byte_for_byte() {
    let debug = profile_debug();
    assert_eq!(debug.len(), Profile::ALL.len());
    for &profile in Profile::ALL {
        let name = profile.as_str();
        let sent = profile.json().trim_end();
        assert_eq!(our_roundtrip(sent).unwrap(), sent, "our serde, {name}");
        assert_eq!(
            serde_json::to_string(&profile.config()).unwrap(),
            sent,
            "config(), {name}"
        );
        assert_eq!(
            our_conversion(sent).unwrap(),
            debug[name],
            "conversion to wreq, {name}"
        );
    }
}

#[test]
fn every_profile_file_is_listed() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("profiles");
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    files.sort();
    let mut listed: Vec<String> = Profile::ALL.iter().map(|p| p.as_str().to_owned()).collect();
    listed.sort();
    assert_eq!(
        files, listed,
        "profiles/ and the `profiles!` list in src/profiles.rs"
    );
}

#[test]
fn recorded_cases_match_the_fork() {
    let (mut accepted, mut rejected) = (0, 0);
    for case in cases() {
        let name = case.name.as_str();
        if WE_REJECT.contains(&name) || WE_ACCEPT.contains(&name) {
            continue;
        }
        if let Some(expected) = &case.fork_output {
            assert_eq!(
                &our_roundtrip(&case.input).unwrap(),
                expected,
                "our serde, {name}"
            );
            assert_eq!(
                &our_conversion(&case.input).unwrap(),
                case.wreq_debug.as_ref().unwrap(),
                "conversion to wreq, {name}"
            );
            accepted += 1;
        } else {
            assert!(our_roundtrip(&case.input).is_err(), "should reject {name}");
            rejected += 1;
        }
    }
    assert!(
        accepted > 0 && rejected > 0,
        "golden file has both kinds of case"
    );
}

#[test]
fn known_differences_stay_as_documented() {
    let cases = cases();
    let case = |name: &str| cases.iter().find(|case| case.name == name).unwrap();
    for name in WE_REJECT {
        assert!(case(name).fork_output.is_some(), "the fork accepts {name}");
        assert!(
            our_roundtrip(&case(name).input).is_err(),
            "we reject {name}"
        );
    }
    for name in WE_ACCEPT {
        assert!(case(name).fork_output.is_none(), "the fork rejects {name}");
        assert!(our_roundtrip(&case(name).input).is_ok(), "we accept {name}");
    }
    // A hand-written short pseudo-header order is kept as sent by our serde, but the conversion
    // completes it through wreq's builder. `recorded_cases_match_the_fork` checks both halves;
    // this only makes sure the case is still recorded.
    assert!(case("hand_written_pseudo_order").fork_output.is_some());
}

#[test]
fn patched_profile_matches_the_same_change_made_in_code() {
    let patch = serde_json::json!({
        "transport": {
            "tls_options": { "record_size_limit": 16385, "grease_enabled": null },
            "http2_options": { "initial_window_size": 65535 }
        }
    });
    let patched = Profile::Chrome145Macos.config().patched(&patch).unwrap();

    let mut expected = Profile::Chrome145Macos.config();
    let tls = expected.transport.tls_options.as_mut().unwrap();
    tls.record_size_limit = Some(16385);
    tls.grease_enabled = None;
    expected
        .transport
        .http2_options
        .as_mut()
        .unwrap()
        .initial_window_size = 65535;

    assert_eq!(
        serde_json::to_string(&patched).unwrap(),
        serde_json::to_string(&expected).unwrap()
    );
}

/// How many distinct names an `http::HeaderMap` holds (24,576 in http 1.5), measured on the
/// real type rather than assumed.
fn header_map_capacity() -> usize {
    let mut map = HeaderMap::new();
    // Twice http's hard ceiling (2^15), so the loop always ends.
    (0..1 << 16)
        .take_while(|i| {
            let name = HeaderName::from_bytes(format!("h{i}").as_bytes()).unwrap();
            map.try_append(name, HeaderValue::from_static("1")).is_ok()
        })
        .count()
}

/// Builds a config with this many distinct header names.
type BuildInput = fn(usize) -> String;

fn headers(count: usize) -> String {
    let pairs: Vec<Vec<Vec<u8>>> = (0..count)
        .map(|i| vec![format!("h{i}").into_bytes(), b"1".to_vec()])
        .collect();
    serde_json::json!({ "headers": pairs }).to_string()
}

fn header_order(count: usize) -> String {
    let names: Vec<Vec<u8>> = (0..count)
        .map(|i| [&[0u8][..], format!("h{i}").as_bytes()].concat())
        .collect();
    serde_json::json!({ "orig_headers": names }).to_string()
}

/// Past the `HeaderMap` limit, the fork's `append` panicked while reading the request body.
/// This crate returns an error instead.
#[test]
fn too_many_distinct_header_names_is_an_error() {
    let limit = header_map_capacity();
    let inputs: [(&str, BuildInput); 2] = [("headers", headers), ("orig_headers", header_order)];
    for (what, build) in inputs {
        assert!(
            our_roundtrip(&build(limit)).is_ok(),
            "{what}: {limit} names fit"
        );
        let error = our_roundtrip(&build(limit + 1)).unwrap_err();
        assert!(
            error.to_string().contains("too many headers"),
            "{what}: {error}"
        );
    }
}
