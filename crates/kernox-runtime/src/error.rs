use std::fmt;

use kernox_core::{CapabilityId, IdentifierError, PluginId, RequirementCardinality, ResolveError};
use semver::Version;
use thiserror::Error;

use crate::LifecyclePhase;

/// Failure while resolving an application graph or its selected Host.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AppResolveError {
    /// The plugin graph was invalid.
    #[error(transparent)]
    Graph(#[from] ResolveError),
    /// A plugin's host-runtime contract was not satisfied.
    #[error(transparent)]
    Host(#[from] crate::HostResolutionError),
}

impl AppResolveError {
    /// Returns the stable machine-readable diagnostic tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Graph(error) => error.tag(),
            Self::Host(error) => error.tag(),
        }
    }
}

/// An invalid compile-time capability contract.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ContractError {
    /// The capability identifier constant is not canonical.
    #[error("capability contract identifier {value:?} is invalid: {source}")]
    InvalidIdentifier {
        /// Rejected identifier.
        value: &'static str,
        /// Identifier validation failure.
        source: IdentifierError,
    },
    /// The capability version constant is not semantic version syntax.
    #[error("capability contract version {value:?} is invalid: {reason}")]
    InvalidVersion {
        /// Rejected version.
        value: &'static str,
        /// Semantic-version parsing failure.
        reason: String,
    },
}

impl ContractError {
    /// Returns the stable machine-readable diagnostic tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::InvalidIdentifier { .. } => "contract.invalid-identifier",
            Self::InvalidVersion { .. } => "contract.invalid-version",
        }
    }
}

/// A staged capability publication failure.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ProvisionError {
    /// The marker type has an invalid contract.
    #[error(transparent)]
    InvalidContract(#[from] ContractError),
    /// One staged set contains the same capability identity twice.
    #[error("capability {capability} was staged more than once")]
    Duplicate {
        /// Duplicate capability identity.
        capability: CapabilityId,
    },
    /// A plugin staged a capability it did not declare.
    #[error("plugin {plugin} staged undeclared capability {capability}")]
    Undeclared {
        /// Publishing plugin.
        plugin: PluginId,
        /// Undeclared capability.
        capability: CapabilityId,
    },
    /// A declared offer was absent from the staged transaction.
    #[error("plugin {plugin} did not stage declared capability {capability}")]
    Missing {
        /// Publishing plugin.
        plugin: PluginId,
        /// Missing capability.
        capability: CapabilityId,
    },
    /// The marker contract and descriptor offer versions differ.
    #[error("plugin {plugin} staged capability {capability} at {actual}, but declared {declared}")]
    VersionMismatch {
        /// Publishing plugin.
        plugin: PluginId,
        /// Capability with mismatched metadata.
        capability: CapabilityId,
        /// Descriptor version.
        declared: Box<Version>,
        /// Marker contract version.
        actual: Box<Version>,
    },
}

impl ProvisionError {
    /// Returns the stable machine-readable diagnostic tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::InvalidContract(error) => error.tag(),
            Self::Duplicate { .. } => "provision.duplicate",
            Self::Undeclared { .. } => "provision.undeclared",
            Self::Missing { .. } => "provision.missing",
            Self::VersionMismatch { .. } => "provision.version-mismatch",
        }
    }
}

/// A typed capability lookup failure during initialization or root export.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AccessError {
    /// The application has already entered shutdown.
    #[error("application capability access is unavailable after shutdown begins")]
    ApplicationUnavailable,
    /// The marker type has an invalid contract.
    #[error(transparent)]
    InvalidContract(#[from] ContractError),
    /// The consumer did not declare this capability requirement.
    #[error("plugin {consumer} attempted to access undeclared capability {capability}")]
    Undeclared {
        /// Accessing plugin.
        consumer: PluginId,
        /// Undeclared capability.
        capability: CapabilityId,
    },
    /// The access method did not agree with the descriptor cardinality.
    #[error(
        "plugin {consumer} used the wrong access mode for capability {capability} ({cardinality:?})"
    )]
    CardinalityMismatch {
        /// Accessing plugin.
        consumer: PluginId,
        /// Requested capability.
        capability: CapabilityId,
        /// Resolved cardinality.
        cardinality: RequirementCardinality,
    },
    /// A selected provision was unexpectedly absent.
    #[error("provider {provider} has no committed capability {capability}")]
    MissingProvision {
        /// Selected provider.
        provider: PluginId,
        /// Missing capability.
        capability: CapabilityId,
    },
    /// The requested marker type differs from the provider's marker type.
    #[error(
        "capability {capability} from {provider} has Rust type {actual}; requested {requested}"
    )]
    TypeMismatch {
        /// Selected provider.
        provider: PluginId,
        /// Capability identity.
        capability: CapabilityId,
        /// Requested marker type.
        requested: &'static str,
        /// Published marker type.
        actual: &'static str,
    },
}

impl AccessError {
    /// Returns the stable machine-readable diagnostic tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::ApplicationUnavailable => "access.application-unavailable",
            Self::InvalidContract(error) => error.tag(),
            Self::Undeclared { .. } => "access.undeclared",
            Self::CardinalityMismatch { .. } => "access.cardinality-mismatch",
            Self::MissingProvision { .. } => "access.missing-provision",
            Self::TypeMismatch { .. } => "access.type-mismatch",
        }
    }
}

/// An expected failure returned by plugin lifecycle code.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct PluginError {
    tag: &'static str,
    message: String,
}

impl PluginError {
    /// Creates a plugin error with a stable static tag and operator-facing text.
    #[must_use]
    pub fn new(tag: &'static str, message: impl Into<String>) -> Self {
        Self { tag, message: message.into() }
    }

    /// Returns the stable machine-readable tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        self.tag
    }

    /// Returns the operator-facing message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// One primary or cleanup lifecycle failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailureRecord {
    /// Lifecycle phase that failed.
    pub phase: LifecyclePhase,
    /// Plugin whose hook or provision contract failed.
    pub plugin: PluginId,
    /// Stable machine-readable error tag.
    pub error_tag: &'static str,
    /// Operator-facing failure text. This is not emitted through observations.
    pub message: String,
}

impl fmt::Display for FailureRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {} failed: {}", self.plugin, self.phase, self.message)
    }
}

/// A boot failure plus every failure observed while rolling it back.
#[derive(Clone, Debug, Error, PartialEq)]
#[error("{primary}")]
pub struct LifecycleFailure {
    /// Failure that initiated rollback.
    pub primary: FailureRecord,
    /// Cleanup failures collected without short-circuiting rollback.
    pub cleanup_failures: Vec<FailureRecord>,
}

/// Terminal shutdown result, cached so shutdown is idempotent.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShutdownReport {
    /// Cleanup failures collected without stopping later hooks.
    pub failures: Vec<FailureRecord>,
}

impl ShutdownReport {
    /// Returns whether every cleanup hook completed successfully.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }
}
