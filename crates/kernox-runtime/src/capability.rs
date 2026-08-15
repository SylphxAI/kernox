use std::{
    any::{Any, TypeId, type_name},
    collections::BTreeMap,
    sync::Arc,
};

use kernox_core::{
    CapabilityId, PluginDescriptor, PluginId, RequirementCardinality, ResolvedGraph,
    ResolvedRequirement,
};
use semver::Version;

use crate::{AccessError, ContractError, ProvisionError, ScopeView};

/// Associates a stable versioned capability identity with a Rust interface.
///
/// Implement this trait on a zero-sized marker type. The interface is normally
/// a `dyn Trait`, allowing providers and consumers to compile independently.
pub trait Capability: Send + Sync + 'static {
    /// Thread-safe interface returned to consumers.
    type Interface: ?Sized + Send + Sync + 'static;

    /// Canonical reverse-domain-like capability identity.
    const ID: &'static str;
    /// Exact semantic version implemented by this marker contract.
    const VERSION: &'static str;
}

/// Parsed runtime representation of a marker's compile-time contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityContract {
    id: CapabilityId,
    version: Version,
    marker_type: TypeId,
    marker_name: &'static str,
}

impl CapabilityContract {
    /// Parses and validates a marker type's constants.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when the identifier or version constant is
    /// malformed.
    pub fn of<C: Capability>() -> Result<Self, ContractError> {
        let id = CapabilityId::new(C::ID)
            .map_err(|source| ContractError::InvalidIdentifier { value: C::ID, source })?;
        let version = Version::parse(C::VERSION).map_err(|source| {
            ContractError::InvalidVersion { value: C::VERSION, reason: source.to_string() }
        })?;
        Ok(Self { id, version, marker_type: TypeId::of::<C>(), marker_name: type_name::<C>() })
    }

    /// Returns the stable capability identity.
    #[must_use]
    pub const fn id(&self) -> &CapabilityId {
        &self.id
    }

    /// Returns the exact interface contract version.
    #[must_use]
    pub const fn version(&self) -> &Version {
        &self.version
    }

    /// Returns the Rust marker type name for diagnostics.
    #[must_use]
    pub const fn marker_name(&self) -> &'static str {
        self.marker_name
    }
}

struct StagedProvision {
    contract: CapabilityContract,
    value: Box<dyn Any + Send + Sync>,
}

/// Transactional collection of capabilities produced by one plugin.
#[derive(Default)]
pub struct ProvisionSet {
    provisions: BTreeMap<CapabilityId, StagedProvision>,
}

impl ProvisionSet {
    /// Creates an empty staged transaction.
    #[must_use]
    pub const fn new() -> Self {
        Self { provisions: BTreeMap::new() }
    }

    /// Stages one typed capability for atomic publication after initialization.
    ///
    /// # Errors
    ///
    /// Returns [`ProvisionError`] for an invalid marker contract or duplicate
    /// capability identity. Declaration and version checks happen at commit.
    pub fn provide<C>(mut self, value: Arc<C::Interface>) -> Result<Self, ProvisionError>
    where
        C: Capability,
    {
        let contract = CapabilityContract::of::<C>()?;
        let id = contract.id.clone();
        if self
            .provisions
            .insert(id.clone(), StagedProvision { contract, value: Box::new(value) })
            .is_some()
        {
            return Err(ProvisionError::Duplicate { capability: id });
        }
        Ok(self)
    }
}

struct CommittedProvision {
    contract: CapabilityContract,
    value: Box<dyn Any + Send + Sync>,
}

#[derive(Default)]
pub(crate) struct Registry {
    provisions: BTreeMap<(PluginId, CapabilityId), CommittedProvision>,
}

impl Registry {
    pub(crate) fn commit(
        &mut self,
        plugin: &PluginId,
        descriptor: &PluginDescriptor,
        staged: ProvisionSet,
    ) -> Result<(), ProvisionError> {
        for (capability, provision) in &staged.provisions {
            let Some(offer) = descriptor.provides().find(|offer| offer.id() == capability) else {
                return Err(ProvisionError::Undeclared {
                    plugin: plugin.clone(),
                    capability: capability.clone(),
                });
            };
            if offer.version() != &provision.contract.version {
                return Err(ProvisionError::VersionMismatch {
                    plugin: plugin.clone(),
                    capability: capability.clone(),
                    declared: Box::new(offer.version().clone()),
                    actual: Box::new(provision.contract.version.clone()),
                });
            }
        }

        for offer in descriptor.provides() {
            if !staged.provisions.contains_key(offer.id()) {
                return Err(ProvisionError::Missing {
                    plugin: plugin.clone(),
                    capability: offer.id().clone(),
                });
            }
        }

        for (capability, provision) in staged.provisions {
            self.provisions.insert(
                (plugin.clone(), capability),
                CommittedProvision { contract: provision.contract, value: provision.value },
            );
        }
        Ok(())
    }

    fn get<C: Capability>(
        &self,
        provider: &PluginId,
        contract: &CapabilityContract,
    ) -> Result<Arc<C::Interface>, AccessError> {
        let key = (provider.clone(), contract.id.clone());
        let Some(provision) = self.provisions.get(&key) else {
            return Err(AccessError::MissingProvision {
                provider: provider.clone(),
                capability: contract.id.clone(),
            });
        };
        if provision.contract.marker_type != contract.marker_type {
            return Err(AccessError::TypeMismatch {
                provider: provider.clone(),
                capability: contract.id.clone(),
                requested: contract.marker_name,
                actual: provision.contract.marker_name,
            });
        }

        provision.value.downcast_ref::<Arc<C::Interface>>().cloned().ok_or_else(|| {
            AccessError::TypeMismatch {
                provider: provider.clone(),
                capability: contract.id.clone(),
                requested: contract.marker_name,
                actual: provision.contract.marker_name,
            }
        })
    }
}

/// Non-owning resolver available only while one plugin initializes.
pub struct InitializationContext<'a> {
    consumer: &'a PluginId,
    graph: &'a ResolvedGraph,
    registry: &'a Registry,
    scope: ScopeView<'a>,
}

impl<'a> InitializationContext<'a> {
    pub(crate) const fn new(
        consumer: &'a PluginId,
        graph: &'a ResolvedGraph,
        registry: &'a Registry,
        scope: ScopeView<'a>,
    ) -> Self {
        Self { consumer, graph, registry, scope }
    }

    /// Returns the non-owning application scope view.
    #[must_use]
    pub const fn scope(&self) -> ScopeView<'a> {
        self.scope
    }

    /// Resolves an exactly-one declared dependency to a direct typed handle.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError`] for an undeclared capability, incorrect access
    /// mode, missing committed provision, or marker type mismatch.
    pub fn require<C: Capability>(&self) -> Result<Arc<C::Interface>, AccessError> {
        let contract = CapabilityContract::of::<C>()?;
        let requirement = self.requirement(&contract.id)?;
        if requirement.cardinality != RequirementCardinality::ExactlyOne {
            return Err(self.cardinality_error(requirement));
        }
        self.registry.get::<C>(&requirement.providers[0], &contract)
    }

    /// Resolves a zero-or-one declared dependency.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError`] for an undeclared capability, incorrect access
    /// mode, missing committed provision, or marker type mismatch.
    pub fn optional<C: Capability>(&self) -> Result<Option<Arc<C::Interface>>, AccessError> {
        let contract = CapabilityContract::of::<C>()?;
        let requirement = self.requirement(&contract.id)?;
        if requirement.cardinality != RequirementCardinality::ZeroOrOne {
            return Err(self.cardinality_error(requirement));
        }
        requirement
            .providers
            .first()
            .map(|provider| self.registry.get::<C>(provider, &contract))
            .transpose()
    }

    /// Resolves every provider of a declared multi-provider dependency.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError`] for an undeclared capability, incorrect access
    /// mode, missing committed provision, or marker type mismatch.
    pub fn all<C: Capability>(&self) -> Result<Vec<Arc<C::Interface>>, AccessError> {
        let contract = CapabilityContract::of::<C>()?;
        let requirement = self.requirement(&contract.id)?;
        if !matches!(
            requirement.cardinality,
            RequirementCardinality::ZeroOrMore | RequirementCardinality::OneOrMore
        ) {
            return Err(self.cardinality_error(requirement));
        }
        requirement
            .providers
            .iter()
            .map(|provider| self.registry.get::<C>(provider, &contract))
            .collect()
    }

    fn requirement(&self, capability: &CapabilityId) -> Result<&ResolvedRequirement, AccessError> {
        self.graph
            .requirements()
            .iter()
            .find(|requirement| {
                requirement.consumer == *self.consumer && requirement.capability == *capability
            })
            .ok_or_else(|| AccessError::Undeclared {
                consumer: self.consumer.clone(),
                capability: capability.clone(),
            })
    }

    fn cardinality_error(&self, requirement: &ResolvedRequirement) -> AccessError {
        AccessError::CardinalityMismatch {
            consumer: self.consumer.clone(),
            capability: requirement.capability.clone(),
            cardinality: requirement.cardinality,
        }
    }
}

pub(crate) fn root_capability<C: Capability>(
    registry: &Registry,
    provider: &PluginId,
) -> Result<Arc<C::Interface>, AccessError> {
    let contract = CapabilityContract::of::<C>()?;
    registry.get::<C>(provider, &contract)
}
