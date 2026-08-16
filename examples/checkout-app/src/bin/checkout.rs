//! Run the checkout example with a selected provider binding.

use std::error::Error;

use futures::executor::block_on;
use kernox_example_checkout_app::{
    CheckoutCapability, PaymentProvider, checkout_plugin_id, compose,
};

fn main() -> Result<(), Box<dyn Error>> {
    let provider = match std::env::args().nth(1).as_deref() {
        None | Some("card") => PaymentProvider::Card,
        Some("wallet") => PaymentProvider::Wallet,
        Some(value) => return Err(format!("unknown provider {value:?}; use card or wallet").into()),
    };

    let mut app = block_on(compose(provider)?.start())?;
    let checkout = app.capability_from::<CheckoutCapability>(&checkout_plugin_id()?)?;
    let receipt = checkout.purchase("kernox-book".to_owned(), 1_999)?;
    println!("provider={} purchased {} for {} cents", receipt.provider, receipt.sku, receipt.cents);

    let report = block_on(app.shutdown());
    if report.is_clean() {
        Ok(())
    } else {
        Err(format!("shutdown had {} failure(s)", report.failures.len()).into())
    }
}
