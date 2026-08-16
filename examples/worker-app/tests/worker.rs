//! Worker host integration behavior.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use kernox_example_worker_app::{WorkerMetricsCapability, compose, worker_plugin_id};

#[tokio::test(flavor = "current_thread")]
async fn worker_runs_and_host_shutdown_drains_it() {
    let mut app =
        compose().expect("composition must resolve").start().await.expect("app must start");
    let metrics = app
        .capability_from::<WorkerMetricsCapability>(&worker_plugin_id().unwrap())
        .expect("metrics root must be available");
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    assert!(metrics.ticks() > 0);
    assert!(app.shutdown().await.is_clean());
}
