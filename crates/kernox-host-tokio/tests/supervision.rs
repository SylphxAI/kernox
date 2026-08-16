//! Concurrency and drain behavior for the Tokio host capability.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::{
    future::pending,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use kernox_core::{CapabilityId, CapabilityRequirement, PluginDescriptor, PluginId};
use kernox_host_tokio::{
    SpawnError, TOKIO_RUNTIME_CAPABILITY_ID, TaskName, TokioTaskConfig, TokioTaskPlugin,
    TokioTasksCapability, tokio_runtime_capability, tokio_task_plugin_id,
};
use kernox_runtime::{
    AppBuilder, BoxFuture, Capability, HostCapability, InitializationContext, Plugin, PluginError,
    ProvisionSet,
};
use semver::{Version, VersionReq};

struct DropProbe(Arc<AtomicBool>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

struct InitTaskPlugin {
    descriptor: PluginDescriptor,
    dropped: Arc<AtomicBool>,
}

impl InitTaskPlugin {
    fn new(dropped: Arc<AtomicBool>) -> Self {
        let descriptor = PluginDescriptor::new(
            PluginId::new("dev.example.a-init-task").unwrap(),
            Version::new(1, 0, 0),
        )
        .require(CapabilityRequirement::exactly_one(
            CapabilityId::new(TokioTasksCapability::ID).unwrap(),
            VersionReq::parse("^1.0").unwrap(),
        ))
        .unwrap();
        Self { descriptor, dropped }
    }
}

impl Plugin for InitTaskPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let tasks = context.require::<TokioTasksCapability>();
        let dropped = Arc::clone(&self.dropped);
        Box::pin(async move {
            let tasks = tasks.map_err(|error| PluginError::new(error.tag(), error.to_string()))?;
            let task_name = TaskName::new("init-stubborn")
                .map_err(|error| PluginError::new(error.tag(), error.to_string()))?;
            tasks
                .spawn(
                    task_name,
                    Box::pin(async move {
                        let _probe = DropProbe(dropped);
                        pending::<()>().await;
                    }),
                )
                .map_err(|error| PluginError::new(error.tag(), error.to_string()))?;
            tokio::task::yield_now().await;
            Ok(ProvisionSet::new())
        })
    }
}

struct InitFailurePlugin {
    descriptor: PluginDescriptor,
}

impl InitFailurePlugin {
    fn new() -> Self {
        let descriptor = PluginDescriptor::new(
            PluginId::new("dev.example.z-init-failure").unwrap(),
            Version::new(1, 0, 0),
        )
        .require(CapabilityRequirement::exactly_one(
            CapabilityId::new(TokioTasksCapability::ID).unwrap(),
            VersionReq::parse("^1.0").unwrap(),
        ))
        .unwrap();
        Self { descriptor }
    }
}

impl Plugin for InitFailurePlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        Box::pin(async { Err(PluginError::new("init.failed", "injected initialization failure")) })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn cooperative_task_observes_cancellation_and_drains() {
    let plugin = TokioTaskPlugin::new(TokioTaskConfig {
        max_tasks: 8,
        drain_timeout: Duration::from_secs(1),
    })
    .unwrap();
    let mut app = AppBuilder::new()
        .host_capability(tokio_runtime_capability().unwrap())
        .plugin(plugin)
        .resolve()
        .unwrap()
        .start()
        .await
        .unwrap();
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
    let mut app = AppBuilder::new()
        .host_capability(tokio_runtime_capability().unwrap())
        .plugin(plugin)
        .resolve()
        .unwrap()
        .start()
        .await
        .unwrap();
    let tasks =
        app.capability_from::<TokioTasksCapability>(&tokio_task_plugin_id().unwrap()).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let drop_probe = Arc::clone(&dropped);
    tasks
        .spawn(
            TaskName::new("stubborn-worker").unwrap(),
            Box::pin(async move {
                let _probe = DropProbe(drop_probe);
                pending::<()>().await;
            }),
        )
        .unwrap();
    assert_eq!(
        tasks.spawn(TaskName::new("over-capacity").unwrap(), Box::pin(async {})).unwrap_err(),
        SpawnError::Capacity
    );

    let report = app.shutdown().await;

    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].error_tag, "tokio-task.drain-timeout");
    assert!(report.failures[0].message.contains("stubborn-worker"));
    assert!(dropped.load(Ordering::Acquire));
    assert!(tasks.pending_tasks().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn panicking_task_fails_closed_without_retaining_its_payload() {
    let plugin = TokioTaskPlugin::new(TokioTaskConfig {
        max_tasks: 8,
        drain_timeout: Duration::from_secs(1),
    })
    .unwrap();
    let mut app = AppBuilder::new()
        .host_capability(tokio_runtime_capability().unwrap())
        .plugin(plugin)
        .resolve()
        .unwrap()
        .start()
        .await
        .unwrap();
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

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn initialization_rollback_drains_tasks_before_returning() {
    let plugin = TokioTaskPlugin::new(TokioTaskConfig {
        max_tasks: 8,
        drain_timeout: Duration::from_secs(1),
    })
    .unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let result = AppBuilder::new()
        .host_capability(tokio_runtime_capability().unwrap())
        .plugin(plugin)
        .plugin(InitTaskPlugin::new(Arc::clone(&dropped)))
        .plugin(InitFailurePlugin::new())
        .resolve()
        .unwrap()
        .start()
        .await;

    let failure = result.err().expect("initialization must fail");
    assert_eq!(failure.primary.error_tag, "init.failed");
    assert_eq!(failure.cleanup_failures.len(), 1);
    assert_eq!(failure.cleanup_failures[0].error_tag, "tokio-task.drain-timeout");
    assert!(dropped.load(Ordering::Acquire));
}

#[test]
fn malformed_names_reject_control_and_format_characters() {
    assert_eq!(TaskName::new("").unwrap_err(), SpawnError::InvalidName);
    assert_eq!(TaskName::new("worker\u{202e}name").unwrap_err(), SpawnError::InvalidName);
    assert_eq!(TaskName::new("worker\u{200b}name").unwrap_err(), SpawnError::InvalidName);
}

#[test]
fn missing_tokio_runtime_fails_before_readiness() {
    let plugin = TokioTaskPlugin::new(TokioTaskConfig::default()).unwrap();
    let failure = futures::executor::block_on(async {
        AppBuilder::new()
            .host_capability(tokio_runtime_capability().unwrap())
            .plugin(plugin)
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .expect("Tokio host must not become ready without a runtime")
    });

    assert_eq!(failure.primary.error_tag, "tokio-task.no-runtime");
    assert!(failure.cleanup_failures.is_empty());
}

#[test]
fn task_spawn_reports_missing_runtime_after_ready_app() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
    let (_app, tasks) = runtime.block_on(async {
        let app = AppBuilder::new()
            .host_capability(tokio_runtime_capability().unwrap())
            .plugin(TokioTaskPlugin::new(TokioTaskConfig::default()).unwrap())
            .resolve()
            .unwrap()
            .start()
            .await
            .unwrap();
        let tasks =
            app.capability_from::<TokioTasksCapability>(&tokio_task_plugin_id().unwrap()).unwrap();
        (app, tasks)
    });
    drop(runtime);

    assert_eq!(
        tasks.spawn(TaskName::new("outside-runtime").unwrap(), Box::pin(async {})).unwrap_err(),
        SpawnError::NoRuntime
    );
    assert!(Arc::strong_count(&tasks) >= 2);
}

#[test]
fn closed_admission_precedes_runtime_lookup() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
    let (mut app, tasks) = runtime.block_on(async {
        AppBuilder::new()
            .host_capability(tokio_runtime_capability().unwrap())
            .plugin(TokioTaskPlugin::new(TokioTaskConfig::default()).unwrap())
            .resolve()
            .unwrap()
            .start()
            .await
            .map(|app| {
                let tasks = app
                    .capability_from::<TokioTasksCapability>(&tokio_task_plugin_id().unwrap())
                    .unwrap();
                (app, tasks)
            })
            .unwrap()
    });
    runtime.block_on(app.shutdown());
    drop(runtime);

    assert_eq!(
        tasks.spawn(TaskName::new("after-shutdown").unwrap(), Box::pin(async {})).unwrap_err(),
        SpawnError::Closed
    );
}

#[test]
fn host_runtime_requirement_fails_before_readiness_and_checks_version() {
    let missing = TokioTaskPlugin::new(TokioTaskConfig::default()).unwrap();
    let error = AppBuilder::new().plugin(missing).resolve().err().expect("host must be declared");
    assert_eq!(error.tag(), "host.missing-capability");

    let incompatible = TokioTaskPlugin::new(TokioTaskConfig::default()).unwrap();
    let error = AppBuilder::new()
        .host_capability(HostCapability::new(
            CapabilityId::new(TOKIO_RUNTIME_CAPABILITY_ID).unwrap(),
            Version::new(2, 0, 0),
        ))
        .plugin(incompatible)
        .resolve()
        .err()
        .expect("host version must match");
    assert_eq!(error.tag(), "host.incompatible-capability");
}
