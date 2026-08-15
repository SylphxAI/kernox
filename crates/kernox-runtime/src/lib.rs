//! Typed provisioning and transactional lifecycle execution for Kernox.
//!
//! Graph work happens before startup. Once ready, callers retain direct typed
//! `Arc` handles; normal calls do not traverse the graph or dispatch events.

mod app;
mod capability;
mod error;
mod observation;
mod plugin;
mod scope;

pub use app::{AppBuilder, InvocationScope, ResolvedApp, RunningApp};
pub use capability::{Capability, CapabilityContract, InitializationContext, ProvisionSet};
pub use error::{
    AccessError, ContractError, FailureRecord, LifecycleFailure, PluginError, ProvisionError,
    ShutdownReport,
};
pub use observation::{
    LifecycleObservation, LifecycleOutcome, LifecyclePhase, NoopObservationSink, ObservationSink,
};
pub use plugin::{BoxFuture, LifecycleContext, Plugin};
pub use scope::{ScopeError, ScopeId, ScopeKind, ScopeState, ScopeView};
