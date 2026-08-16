//! North Star conformance for the unchanged three-plugin reference app.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use futures::executor::block_on;

#[test]
fn reference_application_passes_kernox_conformance() {
    let report = block_on(kernox_testkit::verify_application(
        kernox_example_order_app::compose().expect("reference composition must resolve"),
    ))
    .expect("reference application must pass conformance");

    assert_eq!(report.plugin_count, 3);
    assert_eq!(
        report.source_packages,
        [
            "kernox-example-order-service",
            "kernox-example-order-store",
            "kernox-example-system-clock",
        ]
    );
    assert_eq!(
        report.teardown_order,
        report.startup_order.iter().rev().cloned().collect::<Vec<_>>()
    );
}
