//! Tokio host integration with bounded, named, cancellation-aware task ownership.

use std::{
    collections::BTreeMap,
    fmt,
    panic::AssertUnwindSafe,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::FutureExt;
use kernox_core::{CapabilityId, CapabilityOffer, DescriptorError, PluginDescriptor, PluginId};
use kernox_runtime::{
    BoxFuture, Capability, HostCapability, HostRequirement, InitializationContext,
    LifecycleContext, Plugin, PluginError, ProvisionSet,
};
use semver::{Version, VersionReq};
use thiserror::Error;
use tokio::{runtime::Handle, task::AbortHandle, time::timeout};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

const PLUGIN_ID: &str = "dev.kernox.host.tokio";
const TASK_CAPABILITY_ID: &str = "dev.kernox.host.tokio.tasks";
/// Host property identifying a Tokio runtime execution model.
pub const TOKIO_RUNTIME_CAPABILITY_ID: &str = "dev.kernox.host.tokio.runtime";
const CONTRACT_VERSION: &str = "1.0.0";
const MAX_TASK_NAME_BYTES: usize = 128;

/// Marker for the Tokio supervised-task capability.
pub struct TokioTasksCapability;

impl Capability for TokioTasksCapability {
    type Interface = dyn TokioTasks;

    const ID: &'static str = TASK_CAPABILITY_ID;
    const VERSION: &'static str = CONTRACT_VERSION;
}

/// Opaque task identity within one supervisor instance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId(u64);

impl fmt::Display for TaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "task-{}", self.0)
    }
}

/// Validated bounded task label used only for operations and diagnostics.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TaskName(String);

impl TaskName {
    /// Creates a non-empty bounded task name without control or format characters.
    ///
    /// # Errors
    ///
    /// Returns [`SpawnError::InvalidName`] for malformed input.
    pub fn new(value: impl Into<String>) -> Result<Self, SpawnError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_TASK_NAME_BYTES
            || value.chars().any(is_unsafe_name_char)
        {
            return Err(SpawnError::InvalidName);
        }
        Ok(Self(value))
    }

    /// Returns the validated label.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_unsafe_name_char(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{061c}'
                | '\u{200b}'..='\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{206f}'
                | '\u{feff}'
        )
}

/// Failure to admit a supervised task.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SpawnError {
    /// Name is empty, oversized, or contains control characters.
    #[error("task name is empty, oversized, or contains control characters")]
    InvalidName,
    /// Quiescing has closed task admission.
    #[error("task supervisor is quiescing")]
    Closed,
    /// Configured task count bound was reached.
    #[error("task supervisor capacity is exhausted")]
    Capacity,
    /// No Tokio runtime handle is active on the calling thread.
    #[error("no Tokio runtime is active")]
    NoRuntime,
}

impl SpawnError {
    /// Returns the stable machine-readable diagnostic tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::InvalidName => "tokio-task.invalid-name",
            Self::Closed => "tokio-task.closed",
            Self::Capacity => "tokio-task.capacity",
            Self::NoRuntime => "tokio-task.no-runtime",
        }
    }
}

/// One task still present when a drain budget expired.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingTask {
    /// Opaque supervisor-local identity.
    pub id: TaskId,
    /// Bounded operator-facing name.
    pub name: TaskName,
}

/// A supervised task failure retained without exposing a panic payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskFailure {
    /// Opaque supervisor-local identity.
    pub id: TaskId,
    /// Bounded operator-facing name.
    pub name: TaskName,
    /// Stable machine-readable failure tag.
    pub error_tag: &'static str,
}

/// Object-safe task supervision contract consumed by Tokio-aware plugins.
pub trait TokioTasks: Send + Sync + 'static {
    /// Spawns and tracks one task on the current Tokio runtime.
    ///
    /// # Errors
    ///
    /// Returns [`SpawnError`] when admission is closed, the bound is reached,
    /// or the caller is outside Tokio.
    fn spawn(&self, name: TaskName, future: BoxFuture<'static, ()>) -> Result<TaskId, SpawnError>;

    /// Returns a sticky token cancelled when the application quiesces.
    fn cancellation_token(&self) -> CancellationToken;

    /// Returns a stable identity-ordered snapshot of unfinished tasks.
    fn pending_tasks(&self) -> Vec<PendingTask>;

    /// Returns the first terminal task failure, when one occurred.
    ///
    /// A panic closes task admission and cancels the shared token. The panic
    /// payload is deliberately not retained or exposed.
    fn terminal_failure(&self) -> Option<TaskFailure>;
}

/// Resource and drain policy for [`TokioTaskPlugin`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokioTaskConfig {
    /// Maximum simultaneously unfinished tasks.
    pub max_tasks: usize,
    /// Time allowed for cooperative cancellation before forced abort/report.
    pub drain_timeout: Duration,
}

impl Default for TokioTaskConfig {
    fn default() -> Self {
        Self { max_tasks: 16_384, drain_timeout: Duration::from_secs(30) }
    }
}

/// Invalid host plugin construction.
#[derive(Debug, Error)]
pub enum TokioTaskPluginError {
    /// Zero task capacity can never admit useful work.
    #[error("max_tasks must be greater than zero")]
    ZeroCapacity,
    /// A built-in descriptor contract is invalid.
    #[error(transparent)]
    Descriptor(#[from] DescriptorError),
    /// A built-in stable identifier is invalid.
    #[error("built-in Tokio host identifier is invalid")]
    Identifier,
    /// A built-in semantic version is invalid.
    #[error("built-in Tokio host version is invalid")]
    Version(#[from] semver::Error),
}

/// Kernox plugin that publishes one application-scoped Tokio task supervisor.
pub struct TokioTaskPlugin {
    descriptor: PluginDescriptor,
    config: TokioTaskConfig,
    host_requirements: Vec<HostRequirement>,
    supervisor: Option<Arc<TaskSupervisor>>,
}

impl TokioTaskPlugin {
    /// Creates a task-supervision plugin with explicit resource policy.
    ///
    /// # Errors
    ///
    /// Returns [`TokioTaskPluginError`] for zero capacity or invalid built-in
    /// descriptor constants.
    pub fn new(config: TokioTaskConfig) -> Result<Self, TokioTaskPluginError> {
        if config.max_tasks == 0 {
            return Err(TokioTaskPluginError::ZeroCapacity);
        }
        let id = PluginId::new(PLUGIN_ID).map_err(|_| TokioTaskPluginError::Identifier)?;
        let capability =
            CapabilityId::new(TASK_CAPABILITY_ID).map_err(|_| TokioTaskPluginError::Identifier)?;
        let version = Version::parse(CONTRACT_VERSION)?;
        let runtime = tokio_runtime_capability()?;
        let host_requirements =
            vec![HostRequirement::new(runtime.id().clone(), VersionReq::parse("^1.0")?)];
        let descriptor = PluginDescriptor::new(id, version.clone())
            .provide(CapabilityOffer::new(capability, version))?;
        Ok(Self { descriptor, config, host_requirements, supervisor: None })
    }
}

impl Plugin for TokioTaskPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn host_requirements(&self) -> Vec<HostRequirement> {
        self.host_requirements.clone()
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let supervisor = Arc::new(TaskSupervisor::new(self.config));
        self.supervisor = Some(Arc::clone(&supervisor));
        let interface: Arc<dyn TokioTasks> = supervisor;
        Box::pin(async move {
            ProvisionSet::new()
                .provide::<TokioTasksCapability>(interface)
                .map_err(|error| PluginError::new(error.tag(), error.to_string()))
        })
    }

    fn quiesce<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        if let Some(supervisor) = &self.supervisor {
            supervisor.quiesce();
        }
        Box::pin(async { Ok(()) })
    }

    fn stop<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let supervisor = self.supervisor.as_ref().map(Arc::clone);
        Box::pin(async move {
            let Some(supervisor) = supervisor else {
                return Ok(());
            };
            drain_failure(&supervisor).await.map_or(Ok(()), Err)
        })
    }

    fn dispose<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let supervisor = self.supervisor.take();
        Box::pin(async move {
            if let Some(supervisor) = supervisor {
                supervisor.quiesce();
                if let Some(error) = drain_failure(&supervisor).await {
                    return Err(error);
                }
            }
            Ok(())
        })
    }
}

async fn drain_failure(supervisor: &TaskSupervisor) -> Option<PluginError> {
    let pending = supervisor.drain().await;
    if !supervisor.claim_drain_report() {
        return None;
    }
    if let Some(failure) = supervisor.terminal_failure() {
        let pending_suffix = if pending.is_empty() {
            String::new()
        } else {
            format!("; {} peer task(s) also exceeded drain budget", pending.len())
        };
        Some(PluginError::new(
            failure.error_tag,
            format!(
                "{}:{} terminated unexpectedly{pending_suffix}",
                failure.id,
                failure.name.as_str()
            ),
        ))
    } else if pending.is_empty() {
        None
    } else {
        let names = pending
            .iter()
            .take(16)
            .map(|task| format!("{}:{}", task.id, task.name.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        Some(PluginError::new(
            "tokio-task.drain-timeout",
            format!("{} task(s) exceeded drain budget: {names}", pending.len()),
        ))
    }
}

#[derive(Debug)]
struct TaskRecord {
    name: TaskName,
    abort: Option<AbortHandle>,
}

#[derive(Debug)]
struct State {
    accepting: bool,
    next_id: u64,
    tasks: BTreeMap<TaskId, TaskRecord>,
    terminal_failure: Option<TaskFailure>,
    drain_reported: bool,
}

struct SupervisorInner {
    config: TokioTaskConfig,
    cancellation: CancellationToken,
    tracker: TaskTracker,
    state: Mutex<State>,
}

struct TaskRegistration {
    inner: Arc<SupervisorInner>,
    id: TaskId,
}

impl Drop for TaskRegistration {
    fn drop(&mut self) {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tasks
            .remove(&self.id);
    }
}

impl TaskRegistration {
    fn record_panic(&self) {
        let mut state = self.inner.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.terminal_failure.is_none() {
            let name = state
                .tasks
                .get(&self.id)
                .map_or_else(|| TaskName("unknown-task".to_owned()), |record| record.name.clone());
            state.terminal_failure =
                Some(TaskFailure { id: self.id, name, error_tag: "tokio-task.panicked" });
        }
        state.accepting = false;
        drop(state);
        self.inner.tracker.close();
        self.inner.cancellation.cancel();
    }
}

struct TaskSupervisor {
    inner: Arc<SupervisorInner>,
}

impl TaskSupervisor {
    fn new(config: TokioTaskConfig) -> Self {
        Self {
            inner: Arc::new(SupervisorInner {
                config,
                cancellation: CancellationToken::new(),
                tracker: TaskTracker::new(),
                state: Mutex::new(State {
                    accepting: true,
                    next_id: 1,
                    tasks: BTreeMap::new(),
                    terminal_failure: None,
                    drain_reported: false,
                }),
            }),
        }
    }

    fn quiesce(&self) {
        let mut state = self.inner.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.accepting = false;
        self.inner.tracker.close();
        self.inner.cancellation.cancel();
    }

    async fn drain(&self) -> Vec<PendingTask> {
        if self.inner.tracker.is_empty() {
            return Vec::new();
        }
        if timeout(self.inner.config.drain_timeout, self.inner.tracker.wait()).await.is_ok() {
            return Vec::new();
        }
        let pending = self.pending_tasks();
        self.abort_all();
        self.inner.tracker.wait().await;
        pending
    }

    fn abort_all(&self) {
        let handles: Vec<_> = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tasks
            .values()
            .filter_map(|record| record.abort.clone())
            .collect();
        for handle in handles {
            handle.abort();
        }
    }

    fn claim_drain_report(&self) -> bool {
        let mut state = self.inner.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.drain_reported {
            false
        } else {
            state.drain_reported = true;
            true
        }
    }
}

impl TokioTasks for TaskSupervisor {
    fn spawn(&self, name: TaskName, future: BoxFuture<'static, ()>) -> Result<TaskId, SpawnError> {
        let mut state = self.inner.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if !state.accepting {
            return Err(SpawnError::Closed);
        }
        let handle = Handle::try_current().map_err(|_| SpawnError::NoRuntime)?;
        if state.tasks.len() >= self.inner.config.max_tasks {
            return Err(SpawnError::Capacity);
        }

        let id = TaskId(state.next_id);
        state.next_id = state.next_id.checked_add(1).ok_or(SpawnError::Capacity)?;
        state.tasks.insert(id, TaskRecord { name, abort: None });
        let registration = TaskRegistration { inner: Arc::clone(&self.inner), id };
        let join = self.inner.tracker.spawn_on(
            async move {
                if AssertUnwindSafe(future).catch_unwind().await.is_err() {
                    registration.record_panic();
                }
            },
            &handle,
        );
        if let Some(record) = state.tasks.get_mut(&id) {
            record.abort = Some(join.abort_handle());
        }
        Ok(id)
    }

    fn cancellation_token(&self) -> CancellationToken {
        self.inner.cancellation.clone()
    }

    fn pending_tasks(&self) -> Vec<PendingTask> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tasks
            .iter()
            .map(|(id, record)| PendingTask { id: *id, name: record.name.clone() })
            .collect()
    }

    fn terminal_failure(&self) -> Option<TaskFailure> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .terminal_failure
            .clone()
    }
}

/// Returns the stable identity of [`TokioTaskPlugin`].
///
/// # Errors
///
/// Returns an identifier validation error only if this crate's built-in
/// constant is invalid.
pub fn tokio_task_plugin_id() -> Result<PluginId, kernox_core::IdentifierError> {
    PluginId::new(PLUGIN_ID)
}

/// Returns the host property a Tokio runtime supplies to compatible plugins.
///
/// The selected application Host must pass this capability to
/// [`kernox_runtime::AppBuilder::host_capability`] before resolving a graph
/// containing [`TokioTaskPlugin`].
///
/// # Errors
///
/// Returns a construction error only if this crate's built-in contract is
/// invalid.
pub fn tokio_runtime_capability() -> Result<HostCapability, TokioTaskPluginError> {
    let id = CapabilityId::new(TOKIO_RUNTIME_CAPABILITY_ID)
        .map_err(|_| TokioTaskPluginError::Identifier)?;
    let version = Version::parse(CONTRACT_VERSION)?;
    Ok(HostCapability::new(id, version))
}
