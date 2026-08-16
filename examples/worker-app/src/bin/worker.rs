//! Run the Tokio-supervised worker example.

use std::error::Error;

use kernox_example_worker_app::{WorkerMetricsCapability, compose, worker_plugin_id};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut app = compose()?.start().await?;
    let metrics = app.capability_from::<WorkerMetricsCapability>(&worker_plugin_id()?)?;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    println!("heartbeat ticks before shutdown: {}", metrics.ticks());

    let report = app.shutdown().await;
    if report.is_clean() {
        Ok(())
    } else {
        Err(format!("shutdown had {} failure(s)", report.failures.len()).into())
    }
}
