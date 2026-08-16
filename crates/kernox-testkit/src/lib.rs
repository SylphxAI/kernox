//! Deterministic lifecycle observation and failure injection for plugin tests.

mod conformance;

use std::sync::{Arc, Mutex};

use kernox_core::{PluginDescriptor, PluginId};
use kernox_runtime::{
    BoxFuture, InitializationContext, LifecycleContext, LifecycleObservation, LifecycleOutcome,
    LifecyclePhase, ObservationSink, Plugin, PluginError, ProvisionSet, ScopeId,
};

pub use conformance::{
    ConformanceError, ConformanceReport, MINIMUM_VERIFIED_PLUGINS, verify_application,
};

/// Duration-free lifecycle event suitable for deterministic assertions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedLifecycle {
    /// Plugin whose hook completed.
    pub plugin: PluginId,
    /// Application scope identity.
    pub scope: ScopeId,
    /// Completed phase.
    pub phase: LifecyclePhase,
    /// Success or stable failure tag.
    pub outcome: LifecycleOutcome,
}

/// Thread-safe observation sink with stable snapshots and no wall-clock values.
#[derive(Clone, Default)]
pub struct LifecycleRecorder {
    events: Arc<Mutex<Vec<RecordedLifecycle>>>,
}

impl LifecycleRecorder {
    /// Returns an ordered snapshot without timing noise.
    #[must_use]
    pub fn snapshot(&self) -> Vec<RecordedLifecycle> {
        self.events.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    /// Removes all recorded observations.
    pub fn clear(&self) {
        self.events.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
    }
}

impl ObservationSink for LifecycleRecorder {
    fn record(&self, observation: LifecycleObservation) {
        self.events.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(
            RecordedLifecycle {
                plugin: observation.plugin,
                scope: observation.scope,
                phase: observation.phase,
                outcome: observation.outcome,
            },
        );
    }
}

/// Static expected error injected into one lifecycle hook.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InjectedFailure {
    /// Stable error tag.
    pub tag: &'static str,
    /// Operator-facing failure text.
    pub message: &'static str,
}

impl InjectedFailure {
    fn into_error(self) -> PluginError {
        PluginError::new(self.tag, self.message)
    }
}

/// Independent failure controls for every lifecycle phase.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FailurePlan {
    /// Optional initialization failure.
    pub initialize: Option<InjectedFailure>,
    /// Optional start failure.
    pub start: Option<InjectedFailure>,
    /// Optional quiesce failure.
    pub quiesce: Option<InjectedFailure>,
    /// Optional stop failure.
    pub stop: Option<InjectedFailure>,
    /// Optional dispose failure.
    pub dispose: Option<InjectedFailure>,
}

/// Capability-free plugin for graph order and lifecycle failure tests.
///
/// Use real test providers when the descriptor declares offers; this probe
/// intentionally stages no provisions so descriptor enforcement remains live.
pub struct ProbePlugin {
    descriptor: PluginDescriptor,
    failures: FailurePlan,
}

impl ProbePlugin {
    /// Creates a probe with no injected failures.
    #[must_use]
    pub const fn new(descriptor: PluginDescriptor) -> Self {
        Self {
            descriptor,
            failures: FailurePlan {
                initialize: None,
                start: None,
                quiesce: None,
                stop: None,
                dispose: None,
            },
        }
    }

    /// Replaces the complete failure plan.
    #[must_use]
    pub const fn failures(mut self, failures: FailurePlan) -> Self {
        self.failures = failures;
        self
    }
}

impl Plugin for ProbePlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let failure = self.failures.initialize;
        Box::pin(async move {
            failure.map_or_else(|| Ok(ProvisionSet::new()), |failure| Err(failure.into_error()))
        })
    }

    fn start<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        hook(self.failures.start)
    }

    fn quiesce<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        hook(self.failures.quiesce)
    }

    fn stop<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        hook(self.failures.stop)
    }

    fn dispose<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        hook(self.failures.dispose)
    }
}

fn hook(failure: Option<InjectedFailure>) -> BoxFuture<'static, Result<(), PluginError>> {
    Box::pin(async move { failure.map_or_else(|| Ok(()), |failure| Err(failure.into_error())) })
}
