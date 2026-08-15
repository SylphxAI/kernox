//! Kernox application kernel facade.
//!
//! Start with this crate for composition, typed capabilities, and lifecycle.
//! Host adapters remain opt-in features so the core has no runtime dependency.

/// Deterministic graph contracts and diagnostics.
pub mod core {
    pub use kernox_core::*;
}

/// Typed provisioning, plugin lifecycle, scopes, and observations.
pub mod runtime {
    pub use kernox_runtime::*;
}

#[cfg(feature = "serverless")]
/// Provider-neutral warm application host.
pub mod serverless {
    pub use kernox_host_serverless::*;
}

#[cfg(feature = "tokio")]
/// Tokio supervised task host integration.
pub mod tokio {
    pub use kernox_host_tokio::*;
}

pub use kernox_core::{
    Binding, CapabilityId, CapabilityOffer, CapabilityRequirement, GraphBuilder, GraphLimits,
    PluginDescriptor, PluginId, RequirementCardinality,
};
pub use kernox_runtime::{
    AppBuilder, BoxFuture, Capability, InitializationContext, LifecycleContext, Plugin,
    PluginError, ProvisionSet, ResolvedApp, RunningApp,
};
