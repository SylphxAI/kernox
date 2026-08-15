//! Deterministic composition contracts for the Kernox application kernel.
//!
//! This crate is the pure control-plane core. It validates plugin descriptors,
//! resolves capability providers, and produces a deterministic dependency and
//! lifecycle graph. It performs no I/O and depends on no async runtime.

mod descriptor;
mod error;
mod graph;
mod id;

pub use descriptor::{
    Binding, CapabilityOffer, CapabilityRequirement, PluginDescriptor, PluginSource,
    RequirementCardinality,
};
pub use error::{DescriptorError, IdentifierError, ResolveError};
pub use graph::{
    CompositionSpec, GraphBuilder, GraphLimits, GraphReport, PluginSummary, ResolvedEdge,
    ResolvedGraph, ResolvedRequirement,
};
pub use id::{CapabilityId, PluginId};

/// Schema version emitted by [`GraphReport`].
pub const GRAPH_REPORT_SCHEMA_VERSION: u32 = 1;
