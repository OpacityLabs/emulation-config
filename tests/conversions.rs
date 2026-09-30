//! Our defaults, constructors and header order must agree with wreq.
//!
//! Every expected value comes from running wreq itself. Enum names and their mapping to wreq
//! constants are checked against the recorded fork output in `wire_compat.rs`.

use emulation_config::{
    EmulationConfig, Http1Config, Http2Config, OrigHeaders, TlsConfig,
    http2::{self, PseudoId, SettingId},
};
use pretty_assertions::{assert_eq, assert_ne};
use wreq::{
    header::{HeaderName, OrigHeaderMap},
    http1::Http1Options,
    http2::Http2Options,
    tls::TlsOptions,
};

#[test]
fn defaults_match_wreq() {
    assert_eq!(
        TlsOptions::from(&TlsConfig::default()),
        TlsOptions::default()
    );
    assert_eq!(
        Http1Options::from(&Http1Config::default()),
        Http1Options::default()
    );
    assert_eq!(
        Http2Options::from(&Http2Config::default()),
        Http2Options::default()
    );
    assert_eq!(
        format!("{:?}", wreq::Emulation::from(&EmulationConfig::default())),
        format!("{:?}", wreq::Emulation::default()),
    );
}

#[test]
fn pseudo_order_new_matches_the_wreq_builder() {
    let cases: [&[PseudoId]; 5] = [
        &[],
        &[PseudoId::Path],
        &[
            PseudoId::Method,
            PseudoId::Path,
            PseudoId::Authority,
            PseudoId::Scheme,
        ],
        &[PseudoId::Path, PseudoId::Path, PseudoId::Method],
        PseudoId::ALL,
    ];
    for ids in cases {
        let ours = http2::PseudoOrder::new(ids.iter().copied());
        let built = wreq::http2::PseudoOrder::builder()
            .extend(ids.iter().copied().map(Into::into))
            .build();
        let ours: Vec<wreq::http2::PseudoId> = ours.ids.into_iter().map(Into::into).collect();
        let built: Vec<wreq::http2::PseudoId> = (&built).into_iter().copied().collect();
        assert_eq!(ours, built, "ids: {ids:?}");
    }
}

#[test]
fn settings_order_new_matches_the_wreq_builder() {
    let cases: [&[SettingId]; 4] = [
        &[],
        &[SettingId::InitialWindowSize, SettingId::HeaderTableSize],
        &[
            SettingId::EnablePush,
            SettingId::EnablePush,
            SettingId::MaxFrameSize,
        ],
        SettingId::ALL,
    ];
    for ids in cases {
        let ours = http2::SettingsOrder::new(ids.iter().copied());
        let built = wreq::http2::SettingsOrder::builder()
            .extend(ids.iter().copied().map(Into::into))
            .build();
        let ours: Vec<wreq::http2::SettingId> = ours.ids.into_iter().map(Into::into).collect();
        let built: Vec<wreq::http2::SettingId> = (&built).into_iter().copied().collect();
        assert_eq!(ours, built, "ids: {ids:?}");
    }
}

#[test]
fn priorities_new_matches_the_wreq_builder() {
    let priority = |stream_id: u32, dependency_id: u32| http2::Priority {
        stream_id: http2::StreamId::new(stream_id).unwrap(),
        dependency: http2::StreamDependency {
            dependency_id: http2::StreamId::new(dependency_id).unwrap(),
            weight: 7,
            is_exclusive: false,
        },
    };
    // Stream 0 and the repeated stream 5 are dropped; 40 checks ids past the builder's bitmap.
    let list = [
        priority(5, 0),
        priority(0, 3),
        priority(3, 5),
        priority(5, 1),
        priority(40, 3),
    ];

    let ours = http2::Priorities::new(list);
    let built: Vec<wreq::http2::Priority> = wreq::http2::Priorities::builder()
        .extend(list.map(Into::into))
        .build()
        .into_iter()
        .collect();

    let ours_list: Vec<wreq::http2::Priority> =
        ours.priorities.iter().copied().map(Into::into).collect();
    assert_eq!(ours_list, built);
    let built_max = built
        .iter()
        .map(|p| u32::from(p.stream_id()))
        .max()
        .unwrap_or(0);
    assert_eq!(ours.max_stream_id.get(), built_max);
}

#[test]
fn experimental_settings_new_matches_the_wreq_builder() {
    let settings: Vec<http2::Setting> = SettingId::ALL
        .iter()
        .map(|&id| http2::Setting { id, value: 1 })
        .collect();

    let ours = http2::ExperimentalSettings::new(settings.iter().copied());
    let built = wreq::http2::ExperimentalSettings::builder()
        .extend(
            settings
                .iter()
                .map(|setting| wreq::http2::Setting::from_id(setting.id, setting.value)),
        )
        .build();
    assert_eq!(ours.settings.len(), (&built).into_iter().count());
}

#[test]
fn header_order_built_in_code_matches_wreq() {
    let names = ["Accept", "User-Agent", "not a header", "accept", "cookie"];

    let mut ours: OrigHeaders = names.into_iter().collect();
    ours.insert(HeaderName::from_static("host"));

    let mut built = OrigHeaderMap::new();
    for name in names {
        built.insert(name);
    }
    built.insert(HeaderName::from_static("host"));

    assert_eq!(ours.len(), built.len());
    assert_eq!(
        format!("{:?}", OrigHeaderMap::from(&ours)),
        format!("{built:?}")
    );
}

#[test]
fn header_order_equality_depends_on_order() {
    let first: OrigHeaders = ["Accept", "Cookie"].into_iter().collect();
    let second: OrigHeaders = ["Cookie", "Accept"].into_iter().collect();
    assert_ne!(first, second);
}
