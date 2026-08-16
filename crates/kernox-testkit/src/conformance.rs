//! North Star conformance for verified Kernox applications.

use std::collections::BTreeSet;

use kernox_core::PluginId;
use kernox_runtime::{FailureRecord, LifecycleFailure, ResolvedApp};
use thiserror::Error;

/// Minimum number of separately attributed plugins required by the verified
/// application contract.
pub const MINIMUM_VERIFIED_PLUGINS: usize = 3;

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
    /// The application is too small to exercise the North Star composition path.
    #[error("application has {actual} plugin(s); at least {minimum} are required")]
    TooFewPlugins {
        /// Number of plugins in the graph.
        actual: usize,
        /// Minimum accepted plugin count.
        minimum: usize,
    },
    /// A plugin cannot be tied to an independently attributable source package.
    #[error("plugin {plugin} is missing complete source attribution")]
    MissingSource {
        /// Plugin without a package and repository attribution.
        plugin: PluginId,
    },
    /// Two plugins claim the same source package within one application.
    #[error("source package {package:?} is used by more than one plugin")]
    DuplicateSourcePackage {
        /// Repeated source package name.
        package: String,
    },
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
            Self::TooFewPlugins { .. } => "conformance.too-few-plugins",
            Self::MissingSource { .. } => "conformance.missing-source",
            Self::DuplicateSourcePackage { .. } => "conformance.duplicate-source-package",
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
    let plugin_count = app.graph().plugins().count();
    if plugin_count < MINIMUM_VERIFIED_PLUGINS {
        return Err(ConformanceError::TooFewPlugins {
            actual: plugin_count,
            minimum: MINIMUM_VERIFIED_PLUGINS,
        });
    }

    let mut packages = BTreeSet::new();
    let mut source_packages = Vec::with_capacity(plugin_count);
    for descriptor in app.graph().plugins() {
        let Some(source) = descriptor.source() else {
            return Err(ConformanceError::MissingSource { plugin: descriptor.id().clone() });
        };
        if source.repository().is_none() {
            return Err(ConformanceError::MissingSource { plugin: descriptor.id().clone() });
        }
        let package = source.package().to_owned();
        if !packages.insert(package.clone()) {
            return Err(ConformanceError::DuplicateSourcePackage { package });
        }
        source_packages.push(package);
    }

    let startup_order = app.graph().startup_order().to_vec();
    let teardown_order = app.graph().teardown_order().cloned().collect();
    let mut running = app.start().await.map_err(ConformanceError::Lifecycle)?;
    let shutdown = running.shutdown().await;
    if !shutdown.is_clean() {
        let count = shutdown.failures.len();
        return Err(ConformanceError::Shutdown { count, failures: shutdown.failures });
    }

    Ok(ConformanceReport { plugin_count, source_packages, startup_order, teardown_order })
}
