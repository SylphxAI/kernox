//! North Star conformance for verified Kernox applications.

use kernox_core::{AttributionError, PluginId, verify_graph_attribution};
use kernox_runtime::{FailureRecord, LifecycleFailure, ResolvedApp};
use thiserror::Error;

pub use kernox_core::MINIMUM_VERIFIED_PLUGINS;

/// Evidence returned after one application passes the conformance oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceReport {
    /// Number of plugins in the resolved application graph.
    pub plugin_count: usize,
    /// Unique source package names in stable plugin identity order.
    pub source_packages: Vec<String>,
    /// Deterministic startup order exercised by the runtime.
    pub startup_order: Vec<PluginId>,
    /// Exact reverse teardown order represented by the graph contract.
    pub teardown_order: Vec<PluginId>,
}

/// Stable failure returned when an application does not meet conformance.
#[derive(Debug, Error, PartialEq)]
pub enum ConformanceError {
    /// Graph-level source attribution failed.
    #[error(transparent)]
    Attribution(#[from] AttributionError),
    /// Runtime startup or transactional rollback failed.
    #[error(transparent)]
    Lifecycle(#[from] LifecycleFailure),
    /// Runtime shutdown reported one or more cleanup failures.
    #[error("application shutdown returned {count} cleanup failure(s)")]
    Shutdown {
        /// Number of cleanup failures returned by shutdown.
        count: usize,
        /// Exact cleanup failures retained for the caller.
        failures: Vec<FailureRecord>,
    },
}

impl ConformanceError {
    /// Returns the stable machine-readable failure tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Attribution(error) => error.tag(),
            Self::Lifecycle(_) => "conformance.lifecycle-failed",
            Self::Shutdown { .. } => "conformance.shutdown-failed",
        }
    }
}

/// Runs the North Star composition and lifecycle conformance oracle.
///
/// The application is consumed so this function exercises the exact resolved
/// graph and plugin instances that the caller supplied. A successful result
/// proves source-attributed composition and clean runtime lifecycle behavior;
/// it does not prove package publication, legal ownership, deployment, or live
/// traffic adoption.
///
/// # Errors
///
/// Returns [`ConformanceError`] when the graph lacks the required source
/// attribution, has duplicate source packages, or fails to boot or shut down
/// cleanly.
pub async fn verify_application(app: ResolvedApp) -> Result<ConformanceReport, ConformanceError> {
    let attribution = verify_graph_attribution(app.graph())?;
    let startup_order = app.graph().startup_order().to_vec();
    let teardown_order = app.graph().teardown_order().cloned().collect();
    let mut running = app.start().await.map_err(ConformanceError::Lifecycle)?;
    let shutdown = running.shutdown().await;
    if !shutdown.is_clean() {
        let count = shutdown.failures.len();
        return Err(ConformanceError::Shutdown { count, failures: shutdown.failures });
    }

    Ok(ConformanceReport {
        plugin_count: attribution.plugin_count,
        source_packages: attribution.source_packages,
        startup_order,
        teardown_order,
    })
}
