//! Provider replacement and conformance behavior for the checkout example.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use futures::executor::block_on;
use kernox_example_checkout_app::{
    CheckoutCapability, PaymentProvider, checkout_plugin_id, compose,
};

#[test]
fn domain_service_runs_with_both_explicit_payment_bindings() {
    for provider in [PaymentProvider::Card, PaymentProvider::Wallet] {
        let mut app = block_on(compose(provider).expect("composition must resolve").start())
            .expect("application must start");
        let checkout = app
            .capability_from::<CheckoutCapability>(&checkout_plugin_id().unwrap())
            .expect("checkout root must be available");
        let receipt = checkout.purchase("kernox-book".to_owned(), 1_999).unwrap();

        assert_eq!(receipt.provider, provider.name());
        assert!(block_on(app.shutdown()).is_clean());
    }
}

#[test]
fn four_source_plugins_pass_conformance() {
    let report = block_on(kernox_testkit::verify_application(
        compose(PaymentProvider::Card).expect("composition must resolve"),
    ))
    .expect("checkout graph must pass conformance");

    assert_eq!(report.plugin_count, 4);
    assert_eq!(report.source_packages.len(), 4);
}
