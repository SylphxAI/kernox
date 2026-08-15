//! Public testkit behavior.

#![allow(clippy::unwrap_used)]

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
