//! Provider-neutral serverless reference using the same order domain plugins.

use std::{convert::Infallible, sync::Arc};

use kernox::serverless::{ServerlessConfig, ServerlessHost};
use kernox_example_order_app::{OrderServiceCapability, compose, order_service_plugin_id};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = compose()?.start().await?;
    let mut host = ServerlessHost::new(app, ServerlessConfig::default())?;
    let orders = host.capability_from::<OrderServiceCapability>(&order_service_plugin_id()?)?;
    let order = host
        .invoke(None, move |context| {
            let orders = Arc::clone(&orders);
            Box::pin(async move {
                let order = orders.create("serverless-kernox-book".to_owned());
                Ok::<_, Infallible>((context.scope().id(), order))
            })
        })
        .await?;
    println!("scope {:?} created order {}", order.0, order.1.id);

    let report = host.shutdown().await;
    if report.is_clean() {
        Ok(())
    } else {
        Err(format!("shutdown had {} failure(s)", report.failures.len()).into())
    }
}
