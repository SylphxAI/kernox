//! Long-lived host reference using the shared order domain plugins.

use kernox_example_order_app::{OrderServiceCapability, compose, order_service_plugin_id};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resolved = compose()?;
    let mut app = resolved.start().await?;
    let orders = app.capability_from::<OrderServiceCapability>(&order_service_plugin_id()?)?;
    let order = orders.create("kernox-book".to_owned());
    println!("created order {} for {}", order.id, order.sku);

    let report = app.shutdown().await;
    if report.is_clean() {
        Ok(())
    } else {
        Err(format!("shutdown had {} failure(s)", report.failures.len()).into())
    }
}
