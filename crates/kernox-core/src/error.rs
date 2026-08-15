use semver::{Version, VersionReq};
use thiserror::Error;

use crate::{CapabilityId, PluginId};

/// A stable-identifier validation failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum IdentifierError {
    /// The identifier was empty.
    #[error("identifier is empty")]
    Empty,
    /// The complete identifier exceeded the configured format bound.
    #[error("identifier length {actual} exceeds maximum {maximum}")]
    TooLong {
        /// Observed byte length.
        actual: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// The identifier contained a non-ASCII character.
    #[error("identifier must contain ASCII characters only")]
    NonAscii,
    /// A dotted segment was empty.
    #[error("identifier segment {segment} is empty")]
    EmptySegment {
        /// Zero-based segment index.
        segment: usize,
    },
    /// A dotted segment exceeded its bound.
    #[error("identifier segment {segment} length {actual} exceeds maximum {maximum}")]
    SegmentTooLong {
        /// Zero-based segment index.
        segment: usize,
        /// Observed byte length.
        actual: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// A segment did not start with a lowercase ASCII letter.
    #[error("identifier segment {segment} has an invalid first byte at {byte_index}")]
    InvalidSegmentStart {
        /// Zero-based segment index.
        segment: usize,
        /// Zero-based byte index in the complete identifier.
        byte_index: usize,
    },
    /// A segment ended with a hyphen.
    #[error("identifier segment {segment} has an invalid final byte at {byte_index}")]
    InvalidSegmentEnd {
        /// Zero-based segment index.
        segment: usize,
        /// Zero-based byte index in the complete identifier.
        byte_index: usize,
    },
    /// An unsupported ASCII character was present.
    #[error("identifier contains invalid character {character:?} at byte {byte_index}")]
    InvalidCharacter {
        /// Zero-based byte index.
        byte_index: usize,
        /// Invalid character.
        character: char,
    },
}

impl IdentifierError {
    /// Returns a stable machine-readable diagnostic tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Empty => "identifier.empty",
            Self::TooLong { .. } => "identifier.too-long",
            Self::NonAscii => "identifier.non-ascii",
            Self::EmptySegment { .. } => "identifier.empty-segment",
            Self::SegmentTooLong { .. } => "identifier.segment-too-long",
            Self::InvalidSegmentStart { .. } => "identifier.invalid-segment-start",
            Self::InvalidSegmentEnd { .. } => "identifier.invalid-segment-end",
            Self::InvalidCharacter { .. } => "identifier.invalid-character",
        }
    }
}

/// A plugin descriptor construction failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DescriptorError {
    /// The descriptor offered one capability more than once.
    #[error("plugin {plugin} provides capability {capability} more than once")]
    DuplicateOffer {
        /// Plugin being constructed.
        plugin: PluginId,
        /// Duplicated capability.
        capability: CapabilityId,
    },
    /// The descriptor required one capability more than once.
    #[error("plugin {plugin} requires capability {capability} more than once")]
    DuplicateRequirement {
        /// Plugin being constructed.
        plugin: PluginId,
        /// Duplicated capability.
        capability: CapabilityId,
    },
    /// The descriptor declared the same conflict more than once.
    #[error("plugin {plugin} conflicts with {conflict} more than once")]
    DuplicateConflict {
        /// Plugin being constructed.
        plugin: PluginId,
        /// Duplicated conflicting plugin.
        conflict: PluginId,
    },
    /// A plugin declared a conflict with itself.
    #[error("plugin {plugin} cannot conflict with itself")]
    SelfConflict {
        /// Plugin with the invalid declaration.
        plugin: PluginId,
    },
    /// Source metadata violated its bounded public contract.
    #[error("plugin source {field} is invalid: {reason}")]
    InvalidSource {
        /// Source field name.
        field: &'static str,
        /// Repair-oriented reason.
        reason: &'static str,
    },
}

impl DescriptorError {
    /// Returns a stable machine-readable diagnostic tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::DuplicateOffer { .. } => "descriptor.duplicate-offer",
            Self::DuplicateRequirement { .. } => "descriptor.duplicate-requirement",
            Self::DuplicateConflict { .. } => "descriptor.duplicate-conflict",
            Self::SelfConflict { .. } => "descriptor.self-conflict",
            Self::InvalidSource { .. } => "descriptor.invalid-source",
        }
    }
}

/// A capability graph resolution failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResolveError {
    /// The serialized composition schema major is unsupported.
    #[error("composition schema version {actual} is unsupported; expected {supported}")]
    UnsupportedSchemaVersion {
        /// Observed schema version.
        actual: u32,
        /// Supported schema version.
        supported: u32,
    },
    /// A caller attempted to raise a resource limit beyond Kernox's hard bound.
    #[error("configured {limit} limit {actual} exceeds absolute maximum {maximum}")]
    ConfiguredLimitExceeded {
        /// Stable limit name.
        limit: &'static str,
        /// Requested limit.
        actual: usize,
        /// Absolute accepted maximum.
        maximum: usize,
    },
    /// More plugins were supplied than the configured graph limit.
    #[error("plugin count {actual} exceeds configured maximum {maximum}")]
    PluginLimitExceeded {
        /// Observed plugin count.
        actual: usize,
        /// Configured maximum.
        maximum: usize,
    },
    /// One plugin descriptor exceeded its capability-count limit.
    #[error("plugin {plugin} has {actual} capability declarations; maximum is {maximum}")]
    CapabilityLimitExceeded {
        /// Offending plugin.
        plugin: PluginId,
        /// Observed declaration count.
        actual: usize,
        /// Configured maximum.
        maximum: usize,
    },
    /// The resolved dependency edge count exceeded its bound.
    #[error("resolved edge count {actual} exceeds configured maximum {maximum}")]
    EdgeLimitExceeded {
        /// Observed edge count.
        actual: usize,
        /// Configured maximum.
        maximum: usize,
    },
    /// The same plugin identity appeared more than once.
    #[error("plugin identity {plugin} is duplicated")]
    DuplicatePlugin {
        /// Duplicated plugin identity.
        plugin: PluginId,
    },
    /// Two installed plugins declared a conflict.
    #[error("plugins {plugin} and {conflict} conflict")]
    PluginConflict {
        /// First stable identity.
        plugin: PluginId,
        /// Second stable identity.
        conflict: PluginId,
    },
    /// A plugin both provided and required one capability.
    #[error("plugin {plugin} depends on its own capability {capability}")]
    SelfDependency {
        /// Invalid consumer/provider.
        plugin: PluginId,
        /// Self-provided capability.
        capability: CapabilityId,
    },
    /// No provider existed for a required capability.
    #[error("plugin {consumer} has no provider for required capability {capability} {requirement}")]
    MissingProvider {
        /// Requiring plugin.
        consumer: PluginId,
        /// Required capability.
        capability: CapabilityId,
        /// Required semantic-version range.
        requirement: VersionReq,
    },
    /// Providers existed, but none met the version requirement.
    #[error(
        "plugin {consumer} has no compatible provider for capability {capability} {requirement}"
    )]
    IncompatibleProvider {
        /// Requiring plugin.
        consumer: PluginId,
        /// Required capability.
        capability: CapabilityId,
        /// Required semantic-version range.
        requirement: VersionReq,
        /// Available provider versions, ordered by provider identity.
        available: Vec<(PluginId, Version)>,
    },
    /// A single-provider requirement had multiple compatible candidates.
    #[error("plugin {consumer} has ambiguous providers for capability {capability}")]
    AmbiguousProvider {
        /// Requiring plugin.
        consumer: PluginId,
        /// Required capability.
        capability: CapabilityId,
        /// Compatible candidates in stable order.
        providers: Vec<PluginId>,
    },
    /// A binding named an unknown consumer.
    #[error("binding references unknown consumer {consumer}")]
    UnknownBindingConsumer {
        /// Missing consumer identity.
        consumer: PluginId,
    },
    /// A binding named an unknown provider.
    #[error("binding references unknown provider {provider}")]
    UnknownBindingProvider {
        /// Missing provider identity.
        provider: PluginId,
    },
    /// A binding did not correspond to a declared single-provider requirement.
    #[error("binding for {consumer} capability {capability} has no bindable requirement")]
    UnusedBinding {
        /// Consumer named by the binding.
        consumer: PluginId,
        /// Capability named by the binding.
        capability: CapabilityId,
    },
    /// A binding selected a provider that did not offer the capability.
    #[error("bound provider {provider} does not offer capability {capability} to {consumer}")]
    BoundProviderDoesNotOffer {
        /// Consumer named by the binding.
        consumer: PluginId,
        /// Required capability.
        capability: CapabilityId,
        /// Selected provider.
        provider: PluginId,
    },
    /// A binding selected an incompatible provider version.
    #[error("bound provider {provider} version {provided} does not satisfy {requirement}")]
    IncompatibleBinding {
        /// Consumer named by the binding.
        consumer: PluginId,
        /// Required capability.
        capability: CapabilityId,
        /// Selected provider.
        provider: PluginId,
        /// Provider version.
        provided: Box<Version>,
        /// Required semantic-version range.
        requirement: Box<VersionReq>,
    },
    /// The same binding key appeared more than once.
    #[error("binding for {consumer} capability {capability} is duplicated")]
    DuplicateBinding {
        /// Consumer named by the binding.
        consumer: PluginId,
        /// Capability named by the binding.
        capability: CapabilityId,
    },
    /// A binding attempted to select one member of a multi-provider requirement.
    #[error(
        "binding cannot select one provider for multi-provider capability {capability} on {consumer}"
    )]
    BindingForMultiple {
        /// Consumer named by the binding.
        consumer: PluginId,
        /// Multi-provider capability.
        capability: CapabilityId,
    },
    /// The selected plugin dependency graph contained a cycle.
    #[error("plugin dependency graph contains a cycle")]
    DependencyCycle {
        /// Ordered cycle path with the first identity repeated at the end.
        cycle: Vec<PluginId>,
    },
}

impl ResolveError {
    /// Returns a stable machine-readable diagnostic tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::UnsupportedSchemaVersion { .. } => "graph.unsupported-schema-version",
            Self::ConfiguredLimitExceeded { .. } => "graph.configured-limit-exceeded",
            Self::PluginLimitExceeded { .. } => "graph.plugin-limit",
            Self::CapabilityLimitExceeded { .. } => "graph.capability-limit",
            Self::EdgeLimitExceeded { .. } => "graph.edge-limit",
            Self::DuplicatePlugin { .. } => "graph.duplicate-plugin",
            Self::PluginConflict { .. } => "graph.plugin-conflict",
            Self::SelfDependency { .. } => "graph.self-dependency",
            Self::MissingProvider { .. } => "graph.missing-provider",
            Self::IncompatibleProvider { .. } => "graph.incompatible-provider",
            Self::AmbiguousProvider { .. } => "graph.ambiguous-provider",
            Self::UnknownBindingConsumer { .. } => "graph.unknown-binding-consumer",
            Self::UnknownBindingProvider { .. } => "graph.unknown-binding-provider",
            Self::UnusedBinding { .. } => "graph.unused-binding",
            Self::BoundProviderDoesNotOffer { .. } => "graph.binding-provider-does-not-offer",
            Self::IncompatibleBinding { .. } => "graph.incompatible-binding",
            Self::DuplicateBinding { .. } => "graph.duplicate-binding",
            Self::BindingForMultiple { .. } => "graph.binding-for-multiple",
            Self::DependencyCycle { .. } => "graph.dependency-cycle",
        }
    }
}
