use std::collections::{BTreeMap, BTreeSet};

use semver::{Version, VersionReq};

use crate::{CapabilityId, DescriptorError, PluginId};

const MAX_SOURCE_PACKAGE_LENGTH: usize = 255;
const MAX_SOURCE_REPOSITORY_LENGTH: usize = 2_048;

/// Source attribution for a statically selected plugin.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginSource {
    package: String,
    repository: Option<String>,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PluginSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            package: String,
            repository: Option<String>,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.package, wire.repository).map_err(serde::de::Error::custom)
    }
}

impl PluginSource {
    /// Creates bounded source attribution.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::InvalidSource`] when either field is empty,
    /// oversized, or contains control characters.
    pub fn new(
        package: impl Into<String>,
        repository: Option<String>,
    ) -> Result<Self, DescriptorError> {
        let package = package.into();
        validate_source_field("package", &package, MAX_SOURCE_PACKAGE_LENGTH)?;
        if let Some(repository) = &repository {
            validate_source_field("repository", repository, MAX_SOURCE_REPOSITORY_LENGTH)?;
        }
        Ok(Self { package, repository })
    }

    /// Returns the Cargo package or equivalent source package name.
    #[must_use]
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Returns the optional source repository URL.
    #[must_use]
    pub fn repository(&self) -> Option<&str> {
        self.repository.as_deref()
    }
}

fn validate_source_field(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), DescriptorError> {
    if value.is_empty() {
        return Err(DescriptorError::InvalidSource { field, reason: "must not be empty" });
    }
    if value.len() > maximum {
        return Err(DescriptorError::InvalidSource { field, reason: "exceeds length bound" });
    }
    if value.chars().any(char::is_control) {
        return Err(DescriptorError::InvalidSource {
            field,
            reason: "contains control characters",
        });
    }
    Ok(())
}

/// A versioned capability made available by one plugin.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityOffer {
    id: CapabilityId,
    version: Version,
}

impl CapabilityOffer {
    /// Creates a capability offer.
    #[must_use]
    pub const fn new(id: CapabilityId, version: Version) -> Self {
        Self { id, version }
    }

    /// Returns the stable capability identity.
    #[must_use]
    pub const fn id(&self) -> &CapabilityId {
        &self.id
    }

    /// Returns the provided semantic version.
    #[must_use]
    pub const fn version(&self) -> &Version {
        &self.version
    }
}

/// Provider cardinality required by a plugin.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RequirementCardinality {
    /// Exactly one compatible provider must be selected.
    ExactlyOne,
    /// Zero or one compatible provider may be selected.
    ZeroOrOne,
    /// Every compatible provider is selected; zero is allowed.
    ZeroOrMore,
    /// Every compatible provider is selected; at least one is required.
    OneOrMore,
}

impl RequirementCardinality {
    /// Returns whether a binding may select a single provider.
    #[must_use]
    pub const fn is_bindable(self) -> bool {
        matches!(self, Self::ExactlyOne | Self::ZeroOrOne)
    }

    /// Returns whether absence of a compatible provider is valid.
    #[must_use]
    pub const fn permits_zero(self) -> bool {
        matches!(self, Self::ZeroOrOne | Self::ZeroOrMore)
    }
}

/// A versioned capability dependency declared by a plugin.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityRequirement {
    id: CapabilityId,
    version: VersionReq,
    cardinality: RequirementCardinality,
}

impl CapabilityRequirement {
    /// Creates a capability requirement.
    #[must_use]
    pub const fn new(
        id: CapabilityId,
        version: VersionReq,
        cardinality: RequirementCardinality,
    ) -> Self {
        Self { id, version, cardinality }
    }

    /// Creates an exactly-one capability requirement.
    #[must_use]
    pub const fn exactly_one(id: CapabilityId, version: VersionReq) -> Self {
        Self::new(id, version, RequirementCardinality::ExactlyOne)
    }

    /// Returns the stable capability identity.
    #[must_use]
    pub const fn id(&self) -> &CapabilityId {
        &self.id
    }

    /// Returns the accepted semantic-version range.
    #[must_use]
    pub const fn version(&self) -> &VersionReq {
        &self.version
    }

    /// Returns the provider cardinality.
    #[must_use]
    pub const fn cardinality(&self) -> RequirementCardinality {
        self.cardinality
    }
}

/// Static contract of one plugin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginDescriptor {
    id: PluginId,
    version: Version,
    source: Option<PluginSource>,
    provides: BTreeMap<CapabilityId, CapabilityOffer>,
    requires: BTreeMap<CapabilityId, CapabilityRequirement>,
    conflicts: BTreeSet<PluginId>,
}

#[cfg(feature = "serde")]
impl serde::Serialize for PluginDescriptor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("PluginDescriptor", 6)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("version", &self.version)?;
        state.serialize_field("source", &self.source)?;
        state.serialize_field("provides", &self.provides.values().collect::<Vec<_>>())?;
        state.serialize_field("requires", &self.requires.values().collect::<Vec<_>>())?;
        state.serialize_field("conflicts", &self.conflicts.iter().collect::<Vec<_>>())?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PluginDescriptor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            id: PluginId,
            version: Version,
            source: Option<PluginSource>,
            #[serde(default)]
            provides: Vec<CapabilityOffer>,
            #[serde(default)]
            requires: Vec<CapabilityRequirement>,
            #[serde(default)]
            conflicts: Vec<PluginId>,
        }

        let wire = Wire::deserialize(deserializer)?;
        let mut descriptor = Self::new(wire.id, wire.version);
        if let Some(source) = wire.source {
            descriptor = descriptor.sourced_from(source);
        }
        for offer in wire.provides {
            descriptor = descriptor.provide(offer).map_err(serde::de::Error::custom)?;
        }
        for requirement in wire.requires {
            descriptor = descriptor.require(requirement).map_err(serde::de::Error::custom)?;
        }
        for conflict in wire.conflicts {
            descriptor = descriptor.conflict_with(conflict).map_err(serde::de::Error::custom)?;
        }
        Ok(descriptor)
    }
}

impl PluginDescriptor {
    /// Creates an empty descriptor for a stable plugin identity and version.
    #[must_use]
    pub const fn new(id: PluginId, version: Version) -> Self {
        Self {
            id,
            version,
            source: None,
            provides: BTreeMap::new(),
            requires: BTreeMap::new(),
            conflicts: BTreeSet::new(),
        }
    }

    /// Adds source attribution.
    #[must_use]
    pub fn sourced_from(mut self, source: PluginSource) -> Self {
        self.source = Some(source);
        self
    }

    /// Adds a capability offer.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::DuplicateOffer`] when this descriptor already
    /// provides the same capability identity.
    pub fn provide(mut self, offer: CapabilityOffer) -> Result<Self, DescriptorError> {
        let capability = offer.id.clone();
        if self.provides.insert(capability.clone(), offer).is_some() {
            return Err(DescriptorError::DuplicateOffer { plugin: self.id, capability });
        }
        Ok(self)
    }

    /// Adds a capability requirement.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::DuplicateRequirement`] when this descriptor
    /// already requires the same capability identity.
    pub fn require(mut self, requirement: CapabilityRequirement) -> Result<Self, DescriptorError> {
        let capability = requirement.id.clone();
        if self.requires.insert(capability.clone(), requirement).is_some() {
            return Err(DescriptorError::DuplicateRequirement { plugin: self.id, capability });
        }
        Ok(self)
    }

    /// Declares an incompatible plugin identity.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::SelfConflict`] for the current plugin or
    /// [`DescriptorError::DuplicateConflict`] for a repeated declaration.
    pub fn conflict_with(mut self, conflict: PluginId) -> Result<Self, DescriptorError> {
        if conflict == self.id {
            return Err(DescriptorError::SelfConflict { plugin: self.id });
        }
        if !self.conflicts.insert(conflict.clone()) {
            return Err(DescriptorError::DuplicateConflict { plugin: self.id, conflict });
        }
        Ok(self)
    }

    /// Returns the stable plugin identity.
    #[must_use]
    pub const fn id(&self) -> &PluginId {
        &self.id
    }

    /// Returns the plugin semantic version.
    #[must_use]
    pub const fn version(&self) -> &Version {
        &self.version
    }

    /// Returns optional source attribution.
    #[must_use]
    pub const fn source(&self) -> Option<&PluginSource> {
        self.source.as_ref()
    }

    /// Returns capability offers in stable identity order.
    #[must_use]
    pub fn provides(&self) -> impl ExactSizeIterator<Item = &CapabilityOffer> {
        self.provides.values()
    }

    /// Returns capability requirements in stable identity order.
    #[must_use]
    pub fn requires(&self) -> impl ExactSizeIterator<Item = &CapabilityRequirement> {
        self.requires.values()
    }

    /// Returns conflicting plugin identities in stable order.
    #[must_use]
    pub fn conflicts(&self) -> impl ExactSizeIterator<Item = &PluginId> {
        self.conflicts.iter()
    }

    pub(crate) fn offer(&self, capability: &CapabilityId) -> Option<&CapabilityOffer> {
        self.provides.get(capability)
    }
}

/// Explicit selection of one provider for one consumer capability requirement.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Binding {
    consumer: PluginId,
    capability: CapabilityId,
    provider: PluginId,
}

impl Binding {
    /// Creates a provider binding.
    #[must_use]
    pub const fn new(consumer: PluginId, capability: CapabilityId, provider: PluginId) -> Self {
        Self { consumer, capability, provider }
    }

    /// Returns the requiring plugin.
    #[must_use]
    pub const fn consumer(&self) -> &PluginId {
        &self.consumer
    }

    /// Returns the required capability.
    #[must_use]
    pub const fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    /// Returns the selected provider.
    #[must_use]
    pub const fn provider(&self) -> &PluginId {
        &self.provider
    }
}

#[cfg(all(test, feature = "serde"))]
mod serde_tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn descriptor_deserialization_rejects_duplicate_offers() {
        let input = r#"{
            "id": "dev.example.plugin",
            "version": "1.0.0",
            "source": null,
            "provides": [
                {"id": "dev.example.clock", "version": "1.0.0"},
                {"id": "dev.example.clock", "version": "1.1.0"}
            ],
            "requires": [],
            "conflicts": []
        }"#;

        let error = serde_json::from_str::<PluginDescriptor>(input).unwrap_err();
        assert!(error.to_string().contains("more than once"));
    }

    #[test]
    fn source_deserialization_reapplies_validation() {
        let input = r#"{"package":"bad\npackage","repository":null}"#;

        let error = serde_json::from_str::<PluginSource>(input).unwrap_err();
        assert!(error.to_string().contains("control characters"));
    }
}
