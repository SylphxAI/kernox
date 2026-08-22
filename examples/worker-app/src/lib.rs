//! A domain worker that delegates task ownership to the official Tokio host.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use kernox::core::PluginSource;
use kernox::tokio::{
    SpawnError, TaskName, TokioTaskConfig, TokioTaskPlugin, TokioTasks, TokioTasksCapability,
    tokio_runtime_capability,
};
use kernox::{
    AppBuilder, BoxFuture, Capability, CapabilityId, CapabilityOffer, CapabilityRequirement,
    InitializationContext, LifecycleContext, Plugin, PluginDescriptor, PluginError, PluginId,
    ProvisionSet, ResolvedApp,
};
use semver::{Version, VersionReq};
use thiserror::Error;

const CONTRACT_VERSION: &str = "1.0.0";
const WORKER_PLUGIN_ID: &str = "dev.kernox.examples.worker.heartbeat";
const METRICS_CAPABILITY_ID: &str = "dev.kernox.examples.worker.metrics";

/// Read-only metrics emitted by the example worker.
pub trait WorkerMetrics: Send + Sync {
    /// Returns the number of heartbeat iterations observed so far.
    fn ticks(&self) -> u64;
}

/// Marker for the worker metrics port.
pub struct WorkerMetricsCapability;

impl Capability for WorkerMetricsCapability {
    type Interface = dyn WorkerMetrics;

    const ID: &'static str = METRICS_CAPABILITY_ID;
    const VERSION: &'static str = CONTRACT_VERSION;
}

/// Errors constructing the example's static graph.
#[derive(Debug, Error)]
pub enum ComposeError {
    /// A built-in identifier is invalid.
    #[error(transparent)]
    Identifier(#[from] kernox::core::IdentifierError),
    /// A descriptor declaration is inconsistent.
    #[error(transparent)]
    Descriptor(#[from] kernox::core::DescriptorError),
    /// A built-in semantic version is invalid.
    #[error(transparent)]
    Version(#[from] semver::Error),
    /// The selected graph or Host contract is invalid.
    #[error(transparent)]
    Resolve(#[from] kernox::runtime::AppResolveError),
    /// The official Tokio host plugin could not be constructed.
    #[error(transparent)]
    TokioHost(#[from] kernox::tokio::TokioTaskPluginError),
}

struct Metrics {
    ticks: Arc<AtomicU64>,
}

impl WorkerMetrics for Metrics {
    fn ticks(&self) -> u64 {
        self.ticks.load(Ordering::Relaxed)
    }
}

struct HeartbeatWorker {
    descriptor: PluginDescriptor,
    tasks: Option<Arc<dyn TokioTasks>>,
    ticks: Arc<AtomicU64>,
}

impl HeartbeatWorker {
    fn new() -> Result<Self, ComposeError> {
        let descriptor = PluginDescriptor::new(plugin_id(WORKER_PLUGIN_ID)?, version()?)
            .sourced_from(source("kernox-example-heartbeat-worker")?)
            .provide(CapabilityOffer::new(capability_id(METRICS_CAPABILITY_ID)?, version()?))?
            .require(CapabilityRequirement::exactly_one(
                capability_id(TokioTasksCapability::ID)?,
                VersionReq::parse("^1.0")?,
            ))?;
        Ok(Self { descriptor, tasks: None, ticks: Arc::new(AtomicU64::new(0)) })
    }
}

impl Plugin for HeartbeatWorker {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let tasks = match context.require::<TokioTasksCapability>() {
            Ok(tasks) => tasks,
            Err(error) => {
                return Box::pin(async move { Err(access_failure(error)) });
            }
        };
        self.tasks = Some(Arc::clone(&tasks));
        let metrics: Arc<dyn WorkerMetrics> = Arc::new(Metrics { ticks: Arc::clone(&self.ticks) });
        Box::pin(async move {
            ProvisionSet::new()
                .provide::<WorkerMetricsCapability>(metrics)
                .map_err(|error| PluginError::new(error.tag(), error.to_string()))
        })
    }

    fn start<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let Some(tasks) = self.tasks.as_ref().map(Arc::clone) else {
            return Box::pin(async {
                Err(PluginError::new(
                    "worker.tasks-missing",
                    "Tokio task capability was not initialized",
                ))
            });
        };
        let ticks = Arc::clone(&self.ticks);
        let cancellation = tasks.cancellation_token();
        let task_name = match TaskName::new("heartbeat") {
            Ok(name) => name,
            Err(error) => return Box::pin(async move { Err(spawn_failure(error)) }),
        };
        match tasks.spawn(
            task_name,
            Box::pin(async move {
                while !cancellation.is_cancelled() {
                    ticks.fetch_add(1, Ordering::Relaxed);
                    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                }
            }),
        ) {
            Ok(_) => Box::pin(async { Ok(()) }),
            Err(error) => Box::pin(async move { Err(spawn_failure(error)) }),
        }
    }
}

/// Builds the worker graph with the official Tokio task supervisor.
///
/// # Errors
///
/// Returns [`ComposeError`] when the host plugin, worker descriptor, or graph
/// contract cannot be constructed.
pub fn compose() -> Result<ResolvedApp, ComposeError> {
    Ok(AppBuilder::new()
        .host_capability(tokio_runtime_capability()?)
        .plugin(TokioTaskPlugin::new(TokioTaskConfig {
            max_tasks: 8,
            drain_timeout: std::time::Duration::from_secs(1),
        })?)
        .plugin(HeartbeatWorker::new()?)
        .resolve()?)
}

/// Returns the stable provider identity for the worker metrics export.
///
/// # Errors
///
/// Returns an identifier error only if the built-in identity constant is
/// invalid.
pub fn worker_plugin_id() -> Result<PluginId, kernox::core::IdentifierError> {
    plugin_id(WORKER_PLUGIN_ID)
}

fn plugin_id(value: &str) -> Result<PluginId, kernox::core::IdentifierError> {
    PluginId::new(value)
}

fn capability_id(value: &str) -> Result<CapabilityId, kernox::core::IdentifierError> {
    CapabilityId::new(value)
}

fn version() -> Result<Version, semver::Error> {
    Version::parse(CONTRACT_VERSION)
}

fn source(package: &str) -> Result<PluginSource, kernox::core::DescriptorError> {
    PluginSource::new(
        package,
        Some("https://github.com/SylphxAI/kernox/tree/main/examples/worker-app".to_owned()),
    )
}

fn access_failure(error: kernox::runtime::AccessError) -> PluginError {
    let tag = error.tag();
    let message = error.to_string();
    drop(error);
    PluginError::new(tag, message)
}

fn spawn_failure(error: SpawnError) -> PluginError {
    let tag = error.tag();
    let message = error.to_string();
    PluginError::new(tag, message)
}
