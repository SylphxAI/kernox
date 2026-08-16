//! Public conformance oracle behavior.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use futures::executor::block_on;
use kernox_core::AttributionError;
use kernox_core::{PluginDescriptor, PluginId, PluginSource};
use kernox_runtime::AppBuilder;
use kernox_testkit::{ConformanceError, MINIMUM_VERIFIED_PLUGINS, ProbePlugin, verify_application};
use semver::Version;

#[test]
fn three_source_attributed_plugins_pass_and_report_stable_evidence() {
    let app = AppBuilder::new()
        .plugin(probe("dev.example.alpha", "pkg-alpha"))
        .plugin(probe("dev.example.beta", "pkg-beta"))
        .plugin(probe("dev.example.gamma", "pkg-gamma"))
        .resolve()
        .expect("probe graph must resolve");

    let report = block_on(verify_application(app)).expect("conformance must pass");

    assert_eq!(report.plugin_count, MINIMUM_VERIFIED_PLUGINS);
    assert_eq!(report.source_packages, ["pkg-alpha", "pkg-beta", "pkg-gamma"]);
    assert_eq!(
        report.teardown_order,
        report.startup_order.iter().rev().cloned().collect::<Vec<_>>()
    );
}

#[test]
fn conformance_rejects_fewer_than_three_plugins_before_startup() {
    let app = AppBuilder::new()
        .plugin(probe("dev.example.alpha", "pkg-alpha"))
        .plugin(probe("dev.example.beta", "pkg-beta"))
        .resolve()
        .expect("probe graph must resolve");

    let error = block_on(verify_application(app)).expect_err("two plugins are not enough");

    assert_eq!(error.tag(), "conformance.too-few-plugins");
    assert!(matches!(
        error,
        ConformanceError::Attribution(AttributionError::TooFewPlugins {
            actual: 2,
            minimum: MINIMUM_VERIFIED_PLUGINS
        })
    ));
}

#[test]
fn conformance_rejects_missing_and_duplicate_source_attribution() {
    let missing = AppBuilder::new()
        .plugin(ProbePlugin::new(descriptor("dev.example.missing", None)))
        .plugin(probe("dev.example.beta", "pkg-beta"))
        .plugin(probe("dev.example.gamma", "pkg-gamma"))
        .resolve()
        .expect("probe graph must resolve");
    let missing_error = block_on(verify_application(missing)).expect_err("source is required");
    assert_eq!(missing_error.tag(), "conformance.missing-source");

    let duplicate = AppBuilder::new()
        .plugin(probe("dev.example.alpha", "pkg-shared"))
        .plugin(probe("dev.example.beta", "pkg-shared"))
        .plugin(probe("dev.example.gamma", "pkg-gamma"))
        .resolve()
        .expect("probe graph must resolve");
    let duplicate_error =
        block_on(verify_application(duplicate)).expect_err("source packages must be unique");
    assert_eq!(duplicate_error.tag(), "conformance.duplicate-source-package");
}

fn probe(plugin: &str, package: &str) -> ProbePlugin {
    ProbePlugin::new(descriptor(
        plugin,
        Some(PluginSource::new(package, Some("https://example.invalid/repo".to_owned())).unwrap()),
    ))
}

fn descriptor(plugin: &str, source: Option<PluginSource>) -> PluginDescriptor {
    let descriptor = PluginDescriptor::new(PluginId::new(plugin).unwrap(), Version::new(1, 0, 0));
    source.map_or(descriptor.clone(), |source| descriptor.sourced_from(source))
}
