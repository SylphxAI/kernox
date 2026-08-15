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

/// Absolute plugin count ceiling accepted by the resolver.
pub const ABSOLUTE_MAX_PLUGINS: usize = 65_536;
/// Absolute per-plugin capability declaration ceiling.
pub const ABSOLUTE_MAX_CAPABILITIES_PER_PLUGIN: usize = 4_096;
/// Absolute resolved edge ceiling.
pub const ABSOLUTE_MAX_EDGES: usize = 1_048_576;
