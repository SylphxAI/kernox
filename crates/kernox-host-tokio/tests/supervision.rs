//! Concurrency and drain behavior for the Tokio host capability.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::{future::pending, sync::Arc, time::Duration};

use kernox_host_tokio::{
    SpawnError, TaskName, TokioTaskConfig, TokioTaskPlugin, TokioTasksCapability,
    tokio_task_plugin_id,
};
use kernox_runtime::AppBuilder;

#[tokio::test(flavor = "current_thread")]
async fn cooperative_task_observes_cancellation_and_drains() {
    let plugin = TokioTaskPlugin::new(TokioTaskConfig {
        max_tasks: 8,
        drain_timeout: Duration::from_secs(1),
    })
    .unwrap();
    let mut app = AppBuilder::new().plugin(plugin).resolve().unwrap().start().await.unwrap();
    let tasks =
        app.capability_from::<TokioTasksCapability>(&tokio_task_plugin_id().unwrap()).unwrap();
    let cancellation = tasks.cancellation_token();
    tasks
        .spawn(
            TaskName::new("cooperative-worker").unwrap(),
            Box::pin(async move { cancellation.cancelled().await }),
        )
        .unwrap();

    let report = app.shutdown().await;

    assert!(report.is_clean());
    assert!(tasks.pending_tasks().is_empty());
    assert_eq!(
        tasks.spawn(TaskName::new("late-worker").unwrap(), Box::pin(async {})).unwrap_err(),
        SpawnError::Closed
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn stubborn_task_is_named_reported_and_force_aborted() {
    let plugin = TokioTaskPlugin::new(TokioTaskConfig {
        max_tasks: 1,
        drain_timeout: Duration::from_secs(5),
    })
    .unwrap();
    let mut app = AppBuilder::new().plugin(plugin).resolve().unwrap().start().await.unwrap();
    let tasks =
        app.capability_from::<TokioTasksCapability>(&tokio_task_plugin_id().unwrap()).unwrap();
    tasks.spawn(TaskName::new("stubborn-worker").unwrap(), Box::pin(pending())).unwrap();
    assert_eq!(
        tasks.spawn(TaskName::new("over-capacity").unwrap(), Box::pin(async {})).unwrap_err(),
        SpawnError::Capacity
    );

    let report = app.shutdown().await;
    tokio::task::yield_now().await;

    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].error_tag, "tokio-task.drain-timeout");
    assert!(report.failures[0].message.contains("stubborn-worker"));
    assert!(tasks.pending_tasks().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn panicking_task_fails_closed_without_retaining_its_payload() {
    let plugin = TokioTaskPlugin::new(TokioTaskConfig {
        max_tasks: 8,
        drain_timeout: Duration::from_secs(1),
    })
    .unwrap();
    let mut app = AppBuilder::new().plugin(plugin).resolve().unwrap().start().await.unwrap();
    let tasks =
        app.capability_from::<TokioTasksCapability>(&tokio_task_plugin_id().unwrap()).unwrap();
    let cancellation = tasks.cancellation_token();
    tasks
        .spawn(
            TaskName::new("panicking-worker").unwrap(),
            Box::pin(async { panic!("sentinel-secret-that-must-not-be-retained") }),
        )
        .unwrap();
    tokio::task::yield_now().await;

    let failure = tasks.terminal_failure().expect("panic must be observable");
    assert!(cancellation.is_cancelled());
    assert_eq!(failure.error_tag, "tokio-task.panicked");
    assert_eq!(failure.name.as_str(), "panicking-worker");
    assert!(!format!("{failure:?}").contains("sentinel-secret"));
    assert_eq!(
        tasks.spawn(TaskName::new("late-worker").unwrap(), Box::pin(async {})).unwrap_err(),
        SpawnError::Closed
    );

    let report = app.shutdown().await;
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].error_tag, "tokio-task.panicked");
    assert!(!report.failures[0].message.contains("sentinel-secret"));
}

#[test]
fn malformed_names_and_missing_runtime_fail_without_spawning() {
    assert_eq!(TaskName::new("").unwrap_err(), SpawnError::InvalidName);
    let plugin = TokioTaskPlugin::new(TokioTaskConfig::default()).unwrap();
    let app = futures::executor::block_on(async {
        AppBuilder::new().plugin(plugin).resolve().unwrap().start().await.unwrap()
    });
    let tasks =
        app.capability_from::<TokioTasksCapability>(&tokio_task_plugin_id().unwrap()).unwrap();

    assert_eq!(
        tasks.spawn(TaskName::new("outside-runtime").unwrap(), Box::pin(async {})).unwrap_err(),
        SpawnError::NoRuntime
    );
    assert!(Arc::strong_count(&tasks) >= 2);
}
