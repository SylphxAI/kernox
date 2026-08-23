use kernox_core::{CapabilityId, PluginId};
use semver::{Version, VersionReq};
use thiserror::Error;

/// A versioned runtime property supplied by the selected Host.
///
/// Host capabilities are negotiation metadata, not injectable application
/// provisions. They are checked before plugin initialization and never enter
/// the normal direct-call path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostCapability {
    id: CapabilityId,
    version: Version,
}

impl HostCapability {
    /// Creates one versioned host property.
    #[must_use]
    pub const fn new(id: CapabilityId, version: Version) -> Self {
        Self { id, version }
    }

    /// Returns the stable host-property identity.
    #[must_use]
    pub const fn id(&self) -> &CapabilityId {
        &self.id
    }

    /// Returns the supplied host-property version.
    #[must_use]
    pub const fn version(&self) -> &Version {
        &self.version
    }
}

/// A plugin's versioned requirement on the selected Host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostRequirement {
    id: CapabilityId,
    version: VersionReq,
}

impl HostRequirement {
    /// Creates one host-property requirement.
    #[must_use]
    pub const fn new(id: CapabilityId, version: VersionReq) -> Self {
        Self { id, version }
    }

    /// Returns the required host-property identity.
    #[must_use]
    pub const fn id(&self) -> &CapabilityId {
        &self.id
    }

    /// Returns the accepted host-property version range.
    #[must_use]
    pub const fn version(&self) -> &VersionReq {
        &self.version
    }
}

/// Failure to satisfy a plugin's host-runtime contract before readiness.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HostResolutionError {
    /// The host supplied the same property identity more than once.
    #[error("host capability {capability} was supplied more than once")]
    DuplicateCapability {
        /// Duplicated host-property identity.
        capability: CapabilityId,
    },
    /// A plugin declared the same host property more than once.
    #[error("plugin {plugin} requires host capability {capability} more than once")]
    DuplicateRequirement {
        /// Plugin declaring the duplicate.
        plugin: PluginId,
        /// Duplicated host-property identity.
        capability: CapabilityId,
    },
    /// No supplied host property matched a required identity.
    #[error(
        "plugin {plugin} requires host capability {capability} {requirement}, but the host did not supply it"
    )]
    Missing {
        /// Plugin requiring the host property.
        plugin: PluginId,
        /// Required host-property identity.
        capability: CapabilityId,
        /// Required semantic-version range.
        requirement: VersionReq,
    },
    /// The host supplied the property, but no version matched.
    #[error(
        "plugin {plugin} requires host capability {capability} {requirement}, available versions: {available:?}"
    )]
    Incompatible {
        /// Plugin requiring the host property.
        plugin: PluginId,
        /// Required host-property identity.
        capability: CapabilityId,
        /// Required semantic-version range.
        requirement: VersionReq,
        /// Available versions in stable order.
        available: Vec<Version>,
    },
}

impl HostResolutionError {
    /// Returns the stable machine-readable diagnostic tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::DuplicateCapability { .. } => "host.duplicate-capability",
            Self::DuplicateRequirement { .. } => "host.duplicate-requirement",
            Self::Missing { .. } => "host.missing-capability",
            Self::Incompatible { .. } => "host.incompatible-capability",
        }
    }
}
