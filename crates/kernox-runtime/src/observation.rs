use std::{fmt, sync::Arc, time::Duration};

use kernox_core::PluginId;

use crate::ScopeId;

/// One plugin lifecycle phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LifecyclePhase {
    /// Provision creation.
    Initialize,
    /// Activation after all provisions are committed.
    Start,
    /// Stop accepting new work.
    Quiesce,
    /// Stop owned work.
    Stop,
    /// Release owned resources.
    Dispose,
}

impl fmt::Display for LifecyclePhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Initialize => "initialize",
            Self::Start => "start",
            Self::Quiesce => "quiesce",
            Self::Stop => "stop",
            Self::Dispose => "dispose",
        };
        formatter.write_str(value)
    }
}

/// Lifecycle hook outcome safe for telemetry translation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LifecycleOutcome {
    /// Hook completed successfully.
    Succeeded,
    /// Hook returned an expected error.
    Failed {
        /// Stable error tag. No arbitrary plugin payload is included.
        error_tag: &'static str,
    },
}

/// Provider-neutral lifecycle observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleObservation {
    /// Plugin whose hook completed.
    pub plugin: PluginId,
    /// Application scope identity.
    pub scope: ScopeId,
    /// Completed lifecycle phase.
    pub phase: LifecyclePhase,
    /// Hook outcome.
    pub outcome: LifecycleOutcome,
    /// Monotonic elapsed hook duration.
    pub duration: Duration,
}

/// Synchronous observation boundary implemented by host adapters.
pub trait ObservationSink: Send + Sync + 'static {
    /// Records one completed hook without receiving plugin payloads or secrets.
    fn record(&self, observation: LifecycleObservation);
}

/// Observation sink that intentionally discards events.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopObservationSink;

impl ObservationSink for NoopObservationSink {
    fn record(&self, _observation: LifecycleObservation) {}
}

pub(crate) fn default_sink() -> Arc<dyn ObservationSink> {
    Arc::new(NoopObservationSink)
}
