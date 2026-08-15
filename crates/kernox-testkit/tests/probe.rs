//! Public testkit behavior.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;

use kernox_core::{PluginDescriptor, PluginId};
use kernox_runtime::{AppBuilder, LifecycleOutcome, LifecyclePhase};
use kernox_testkit::{FailurePlan, InjectedFailure, LifecycleRecorder, ProbePlugin};
use semver::Version;

#[test]
fn records_deterministic_failure_and_complete_rollback() {
    futures::executor::block_on(async {
        let recorder = LifecycleRecorder::default();
        let descriptor = PluginDescriptor::new(
            PluginId::new("dev.example.probe").unwrap(),
            Version::new(1, 0, 0),
        );
        let probe = ProbePlugin::new(descriptor).failures(FailurePlan {
            start: Some(InjectedFailure { tag: "probe.start-failed", message: "injected" }),
            ..FailurePlan::default()
        });
        let error = AppBuilder::new()
            .plugin(probe)
            .observation_sink(Arc::new(recorder.clone()))
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .unwrap();

        assert_eq!(error.primary.error_tag, "probe.start-failed");
        let events = recorder.snapshot();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].phase, LifecyclePhase::Initialize);
        assert_eq!(events[1].outcome, LifecycleOutcome::Failed { error_tag: "probe.start-failed" });
        assert_eq!(events[4].phase, LifecyclePhase::Dispose);
    });
}

#[test]
fn initialization_failure_is_observed_and_disposed() {
    futures::executor::block_on(async {
        let recorder = LifecycleRecorder::default();
        let probe = ProbePlugin::new(descriptor()).failures(FailurePlan {
            initialize: Some(InjectedFailure {
                tag: "probe.initialize-failed",
                message: "injected",
            }),
            ..FailurePlan::default()
        });
        let error = AppBuilder::new()
            .plugin(probe)
            .observation_sink(Arc::new(recorder.clone()))
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .expect("initialization must fail");

        assert_eq!(error.primary.error_tag, "probe.initialize-failed");
        let events = recorder.snapshot();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].phase, LifecyclePhase::Initialize);
        assert_eq!(events[1].phase, LifecyclePhase::Dispose);
    });
}

#[test]
fn shutdown_preserves_every_cleanup_failure_and_is_idempotent() {
    futures::executor::block_on(async {
        let recorder = LifecycleRecorder::default();
        let probe = ProbePlugin::new(descriptor()).failures(FailurePlan {
            quiesce: Some(InjectedFailure { tag: "probe.quiesce-failed", message: "injected" }),
            stop: Some(InjectedFailure { tag: "probe.stop-failed", message: "injected" }),
            dispose: Some(InjectedFailure { tag: "probe.dispose-failed", message: "injected" }),
            ..FailurePlan::default()
        });
        let mut app = AppBuilder::new()
            .plugin(probe)
            .observation_sink(Arc::new(recorder.clone()))
            .resolve()
            .unwrap()
            .start()
            .await
            .unwrap();

        let first = app.shutdown().await;
        let event_count = recorder.snapshot().len();
        let second = app.shutdown().await;

        assert_eq!(first, second);
        assert_eq!(recorder.snapshot().len(), event_count);
        assert_eq!(
            first.failures.iter().map(|failure| failure.error_tag).collect::<Vec<_>>(),
            ["probe.quiesce-failed", "probe.stop-failed", "probe.dispose-failed"]
        );
    });
}

fn descriptor() -> PluginDescriptor {
    PluginDescriptor::new(PluginId::new("dev.example.probe").unwrap(), Version::new(1, 0, 0))
}
