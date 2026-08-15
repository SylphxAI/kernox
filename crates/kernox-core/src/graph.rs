use std::collections::{BTreeMap, BTreeSet};

use semver::Version;

use crate::{
    Binding, CapabilityId, CapabilityRequirement, GRAPH_REPORT_SCHEMA_VERSION, PluginDescriptor,
    PluginId, RequirementCardinality, ResolveError,
};

/// Resource limits applied during graph resolution.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphLimits {
    /// Maximum number of plugins in one application graph.
    pub max_plugins: usize,
    /// Maximum combined offers and requirements on one plugin.
    pub max_capabilities_per_plugin: usize,
    /// Maximum resolved capability edges.
    pub max_edges: usize,
}

impl Default for GraphLimits {
    fn default() -> Self {
        Self { max_plugins: 4_096, max_capabilities_per_plugin: 512, max_edges: 131_072 }
    }
}

/// Serializable composition input for inspection and offline validation.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionSpec {
    /// Input schema version. Version 1 is the only currently accepted version.
    pub schema_version: u32,
    /// Graph resource limits.
    pub limits: GraphLimits,
    /// Plugin descriptors.
    pub plugins: Vec<PluginDescriptor>,
    /// Explicit single-provider bindings.
    pub bindings: Vec<Binding>,
}

impl Default for CompositionSpec {
    fn default() -> Self {
        Self {
            schema_version: GRAPH_REPORT_SCHEMA_VERSION,
            limits: GraphLimits::default(),
            plugins: Vec::new(),
            bindings: Vec::new(),
        }
    }
}

/// Builder for a deterministic capability graph.
#[derive(Clone, Debug, Default)]
pub struct GraphBuilder {
    spec: CompositionSpec,
}

impl GraphBuilder {
    /// Creates an empty graph builder with production defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a builder from a serializable specification.
    #[must_use]
    pub const fn from_spec(spec: CompositionSpec) -> Self {
        Self { spec }
    }

    /// Replaces graph resource limits.
    #[must_use]
    pub const fn with_limits(mut self, limits: GraphLimits) -> Self {
        self.spec.limits = limits;
        self
    }

    /// Adds a plugin descriptor.
    #[must_use]
    pub fn plugin(mut self, descriptor: PluginDescriptor) -> Self {
        self.spec.plugins.push(descriptor);
        self
    }

    /// Adds an explicit single-provider binding.
    #[must_use]
    pub fn binding(mut self, binding: Binding) -> Self {
        self.spec.bindings.push(binding);
        self
    }

    /// Resolves the immutable graph or returns one repair-oriented failure.
    ///
    /// # Errors
    ///
    /// Returns [`ResolveError`] when limits, identities, conflicts, bindings,
    /// provider compatibility, or dependency acyclicity are invalid.
    pub fn resolve(self) -> Result<ResolvedGraph, ResolveError> {
        resolve(self.spec)
    }
}

/// One resolved requirement and its selected providers.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRequirement {
    /// Requiring plugin.
    pub consumer: PluginId,
    /// Required capability.
    pub capability: CapabilityId,
    /// Requirement cardinality.
    pub cardinality: RequirementCardinality,
    /// Selected providers in stable identity order.
    pub providers: Vec<PluginId>,
}

/// One selected provider-to-consumer capability edge.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResolvedEdge {
    /// Providing plugin.
    pub provider: PluginId,
    /// Requiring plugin.
    pub consumer: PluginId,
    /// Capability responsible for the edge.
    pub capability: CapabilityId,
}

/// Stable public summary of one plugin in a graph report.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginSummary {
    /// Plugin identity.
    pub id: PluginId,
    /// Plugin semantic version.
    pub version: Version,
}

/// Versioned inspectable projection of a resolved graph.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphReport {
    /// Report schema version.
    pub schema_version: u32,
    /// Plugins in stable identity order.
    pub plugins: Vec<PluginSummary>,
    /// Resolved requirements in stable consumer/capability order.
    pub requirements: Vec<ResolvedRequirement>,
    /// Capability-attributed edges in stable order.
    pub edges: Vec<ResolvedEdge>,
    /// Deterministic lifecycle startup order.
    pub startup_order: Vec<PluginId>,
    /// Exact reverse lifecycle teardown order.
    pub teardown_order: Vec<PluginId>,
}

/// Immutable validated application graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGraph {
    plugins: BTreeMap<PluginId, PluginDescriptor>,
    requirements: Vec<ResolvedRequirement>,
    edges: Vec<ResolvedEdge>,
    startup_order: Vec<PluginId>,
}

impl ResolvedGraph {
    /// Returns a plugin descriptor by stable identity.
    #[must_use]
    pub fn plugin(&self, id: &PluginId) -> Option<&PluginDescriptor> {
        self.plugins.get(id)
    }

    /// Returns all plugins in stable identity order.
    #[must_use]
    pub fn plugins(&self) -> impl ExactSizeIterator<Item = &PluginDescriptor> {
        self.plugins.values()
    }

    /// Returns resolved requirements in stable consumer/capability order.
    #[must_use]
    pub fn requirements(&self) -> &[ResolvedRequirement] {
        &self.requirements
    }

    /// Returns capability-attributed edges in stable order.
    #[must_use]
    pub fn edges(&self) -> &[ResolvedEdge] {
        &self.edges
    }

    /// Returns deterministic startup order.
    #[must_use]
    pub fn startup_order(&self) -> &[PluginId] {
        &self.startup_order
    }

    /// Returns teardown order as an iterator without allocating.
    #[must_use]
    pub fn teardown_order(&self) -> impl ExactSizeIterator<Item = &PluginId> {
        self.startup_order.iter().rev()
    }

    /// Produces the versioned inspection projection.
    #[must_use]
    pub fn report(&self) -> GraphReport {
        GraphReport {
            schema_version: GRAPH_REPORT_SCHEMA_VERSION,
            plugins: self
                .plugins
                .values()
                .map(|descriptor| PluginSummary {
                    id: descriptor.id().clone(),
                    version: descriptor.version().clone(),
                })
                .collect(),
            requirements: self.requirements.clone(),
            edges: self.edges.clone(),
            startup_order: self.startup_order.clone(),
            teardown_order: self.startup_order.iter().rev().cloned().collect(),
        }
    }
}

#[derive(Clone, Debug)]
struct ProviderCandidate {
    plugin: PluginId,
    version: Version,
}

fn resolve(spec: CompositionSpec) -> Result<ResolvedGraph, ResolveError> {
    validate_schema_version(spec.schema_version)?;

    validate_plugin_limit(spec.plugins.len(), spec.limits.max_plugins)?;

    let mut plugins = BTreeMap::new();
    for descriptor in spec.plugins {
        let id = descriptor.id().clone();
        let declaration_count = descriptor.provides().len() + descriptor.requires().len();
        if declaration_count > spec.limits.max_capabilities_per_plugin {
            return Err(ResolveError::CapabilityLimitExceeded {
                plugin: id,
                actual: declaration_count,
                maximum: spec.limits.max_capabilities_per_plugin,
            });
        }
        if plugins.insert(id.clone(), descriptor).is_some() {
            return Err(ResolveError::DuplicatePlugin { plugin: id });
        }
    }

    validate_conflicts(&plugins)?;
    validate_no_self_dependencies(&plugins)?;

    let mut providers: BTreeMap<CapabilityId, Vec<ProviderCandidate>> = BTreeMap::new();
    for descriptor in plugins.values() {
        for offer in descriptor.provides() {
            providers.entry(offer.id().clone()).or_default().push(ProviderCandidate {
                plugin: descriptor.id().clone(),
                version: offer.version().clone(),
            });
        }
    }

    let mut bindings = BTreeMap::new();
    for binding in spec.bindings {
        if !plugins.contains_key(binding.consumer()) {
            return Err(ResolveError::UnknownBindingConsumer {
                consumer: binding.consumer().clone(),
            });
        }
        if !plugins.contains_key(binding.provider()) {
            return Err(ResolveError::UnknownBindingProvider {
                provider: binding.provider().clone(),
            });
        }
        let key = (binding.consumer().clone(), binding.capability().clone());
        if bindings.insert(key, binding.provider().clone()).is_some() {
            return Err(ResolveError::DuplicateBinding {
                consumer: binding.consumer().clone(),
                capability: binding.capability().clone(),
            });
        }
    }

    let mut resolved_requirements = Vec::new();
    let mut resolved_edges = BTreeSet::new();
    let mut used_bindings = BTreeSet::new();

    for descriptor in plugins.values() {
        for requirement in descriptor.requires() {
            let key = (descriptor.id().clone(), requirement.id().clone());
            let binding = bindings.get(&key);
            let selected = select_providers(
                descriptor.id(),
                requirement,
                binding,
                providers.get(requirement.id()).map(Vec::as_slice).unwrap_or_default(),
                &plugins,
            )?;

            if binding.is_some() {
                used_bindings.insert(key);
            }

            for provider in &selected {
                resolved_edges.insert(ResolvedEdge {
                    provider: provider.clone(),
                    consumer: descriptor.id().clone(),
                    capability: requirement.id().clone(),
                });
                if resolved_edges.len() > spec.limits.max_edges {
                    return Err(ResolveError::EdgeLimitExceeded {
                        actual: resolved_edges.len(),
                        maximum: spec.limits.max_edges,
                    });
                }
            }

            resolved_requirements.push(ResolvedRequirement {
                consumer: descriptor.id().clone(),
                capability: requirement.id().clone(),
                cardinality: requirement.cardinality(),
                providers: selected,
            });
        }
    }

    if let Some((consumer, capability)) = bindings.keys().find(|key| !used_bindings.contains(*key))
    {
        return Err(ResolveError::UnusedBinding {
            consumer: consumer.clone(),
            capability: capability.clone(),
        });
    }

    let edges: Vec<_> = resolved_edges.into_iter().collect();
    let startup_order = topological_order(plugins.keys(), &edges)?;

    Ok(ResolvedGraph { plugins, requirements: resolved_requirements, edges, startup_order })
}

fn validate_schema_version(actual: u32) -> Result<(), ResolveError> {
    if actual == GRAPH_REPORT_SCHEMA_VERSION {
        return Ok(());
    }

    Err(ResolveError::UnsupportedSchemaVersion { actual, supported: GRAPH_REPORT_SCHEMA_VERSION })
}

fn validate_plugin_limit(actual: usize, maximum: usize) -> Result<(), ResolveError> {
    if actual <= maximum {
        return Ok(());
    }

    Err(ResolveError::PluginLimitExceeded { actual, maximum })
}

fn validate_conflicts(plugins: &BTreeMap<PluginId, PluginDescriptor>) -> Result<(), ResolveError> {
    for descriptor in plugins.values() {
        for conflict in descriptor.conflicts() {
            if plugins.contains_key(conflict) {
                let (plugin, conflict) = if descriptor.id() < conflict {
                    (descriptor.id().clone(), conflict.clone())
                } else {
                    (conflict.clone(), descriptor.id().clone())
                };
                return Err(ResolveError::PluginConflict { plugin, conflict });
            }
        }
    }
    Ok(())
}

fn validate_no_self_dependencies(
    plugins: &BTreeMap<PluginId, PluginDescriptor>,
) -> Result<(), ResolveError> {
    for descriptor in plugins.values() {
        for requirement in descriptor.requires() {
            if descriptor.offer(requirement.id()).is_some() {
                return Err(ResolveError::SelfDependency {
                    plugin: descriptor.id().clone(),
                    capability: requirement.id().clone(),
                });
            }
        }
    }
    Ok(())
}

fn select_providers(
    consumer: &PluginId,
    requirement: &CapabilityRequirement,
    binding: Option<&PluginId>,
    candidates: &[ProviderCandidate],
    plugins: &BTreeMap<PluginId, PluginDescriptor>,
) -> Result<Vec<PluginId>, ResolveError> {
    if binding.is_some() && !requirement.cardinality().is_bindable() {
        return Err(ResolveError::BindingForMultiple {
            consumer: consumer.clone(),
            capability: requirement.id().clone(),
        });
    }

    if let Some(provider) = binding {
        let descriptor = &plugins[provider];
        let Some(offer) = descriptor.offer(requirement.id()) else {
            return Err(ResolveError::BoundProviderDoesNotOffer {
                consumer: consumer.clone(),
                capability: requirement.id().clone(),
                provider: provider.clone(),
            });
        };
        if !requirement.version().matches(offer.version()) {
            return Err(ResolveError::IncompatibleBinding {
                consumer: consumer.clone(),
                capability: requirement.id().clone(),
                provider: provider.clone(),
                provided: Box::new(offer.version().clone()),
                requirement: Box::new(requirement.version().clone()),
            });
        }
        return Ok(vec![provider.clone()]);
    }

    let compatible: Vec<_> = candidates
        .iter()
        .filter(|candidate| requirement.version().matches(&candidate.version))
        .map(|candidate| candidate.plugin.clone())
        .collect();

    match requirement.cardinality() {
        RequirementCardinality::ExactlyOne | RequirementCardinality::ZeroOrOne => {
            match compatible.as_slice() {
                [provider] => Ok(vec![provider.clone()]),
                [] if requirement.cardinality().permits_zero() => Ok(Vec::new()),
                [] if candidates.is_empty() => Err(ResolveError::MissingProvider {
                    consumer: consumer.clone(),
                    capability: requirement.id().clone(),
                    requirement: requirement.version().clone(),
                }),
                [] => Err(ResolveError::IncompatibleProvider {
                    consumer: consumer.clone(),
                    capability: requirement.id().clone(),
                    requirement: requirement.version().clone(),
                    available: candidates
                        .iter()
                        .map(|candidate| (candidate.plugin.clone(), candidate.version.clone()))
                        .collect(),
                }),
                _ => Err(ResolveError::AmbiguousProvider {
                    consumer: consumer.clone(),
                    capability: requirement.id().clone(),
                    providers: compatible,
                }),
            }
        }
        RequirementCardinality::OneOrMore if compatible.is_empty() && candidates.is_empty() => {
            Err(ResolveError::MissingProvider {
                consumer: consumer.clone(),
                capability: requirement.id().clone(),
                requirement: requirement.version().clone(),
            })
        }
        RequirementCardinality::OneOrMore if compatible.is_empty() => {
            Err(ResolveError::IncompatibleProvider {
                consumer: consumer.clone(),
                capability: requirement.id().clone(),
                requirement: requirement.version().clone(),
                available: candidates
                    .iter()
                    .map(|candidate| (candidate.plugin.clone(), candidate.version.clone()))
                    .collect(),
            })
        }
        RequirementCardinality::ZeroOrMore | RequirementCardinality::OneOrMore => Ok(compatible),
    }
}

fn topological_order<'a>(
    plugin_ids: impl Iterator<Item = &'a PluginId>,
    edges: &[ResolvedEdge],
) -> Result<Vec<PluginId>, ResolveError> {
    let nodes: BTreeSet<_> = plugin_ids.cloned().collect();
    let mut indegree: BTreeMap<PluginId, usize> = nodes.iter().cloned().map(|id| (id, 0)).collect();
    let mut adjacency: BTreeMap<PluginId, BTreeSet<PluginId>> = BTreeMap::new();

    for edge in edges {
        let inserted =
            adjacency.entry(edge.provider.clone()).or_default().insert(edge.consumer.clone());
        if inserted {
            indegree.entry(edge.consumer.clone()).and_modify(|degree| *degree += 1);
        }
    }

    let mut ready: BTreeSet<_> =
        indegree.iter().filter_map(|(id, degree)| (*degree == 0).then_some(id.clone())).collect();
    let mut order = Vec::with_capacity(nodes.len());

    while let Some(next) = ready.pop_first() {
        order.push(next.clone());
        if let Some(consumers) = adjacency.get(&next) {
            for consumer in consumers {
                if let Some(degree) = indegree.get_mut(consumer) {
                    *degree -= 1;
                    if *degree == 0 {
                        ready.insert(consumer.clone());
                    }
                }
            }
        }
    }

    if order.len() != nodes.len() {
        let remaining: BTreeSet<_> =
            indegree.into_iter().filter_map(|(id, degree)| (degree > 0).then_some(id)).collect();
        return Err(ResolveError::DependencyCycle { cycle: find_cycle(&remaining, &adjacency) });
    }

    Ok(order)
}

fn find_cycle(
    remaining: &BTreeSet<PluginId>,
    adjacency: &BTreeMap<PluginId, BTreeSet<PluginId>>,
) -> Vec<PluginId> {
    #[derive(Clone, Copy, Eq, PartialEq)]
    enum State {
        Visiting,
        Done,
    }

    let mut states = BTreeMap::new();

    for start in remaining {
        if states.contains_key(start) {
            continue;
        }

        let mut stack = vec![(start.clone(), 0_usize)];
        let mut path = vec![start.clone()];
        let mut positions = BTreeMap::from([(start.clone(), 0_usize)]);
        states.insert(start.clone(), State::Visiting);

        while let Some((node, next_index)) = stack.last_mut() {
            let neighbours: Vec<_> = adjacency
                .get(node)
                .into_iter()
                .flatten()
                .filter(|candidate| remaining.contains(*candidate))
                .cloned()
                .collect();

            if *next_index >= neighbours.len() {
                let finished = node.clone();
                stack.pop();
                path.pop();
                positions.remove(&finished);
                states.insert(finished, State::Done);
                continue;
            }

            let next = neighbours[*next_index].clone();
            *next_index += 1;

            match states.get(&next) {
                Some(State::Visiting) => {
                    if let Some(position) = positions.get(&next).copied() {
                        let mut cycle = path[position..].to_vec();
                        cycle.push(next);
                        return cycle;
                    }
                }
                Some(State::Done) => {}
                None => {
                    positions.insert(next.clone(), path.len());
                    path.push(next.clone());
                    states.insert(next.clone(), State::Visiting);
                    stack.push((next, 0));
                }
            }
        }
    }

    remaining.iter().next().cloned().into_iter().collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use proptest::prelude::*;
    use semver::{Version, VersionReq};

    use super::*;
    use crate::{CapabilityOffer, CapabilityRequirement, PluginDescriptor};

    fn plugin(value: &str) -> PluginId {
        PluginId::new(value).unwrap()
    }

    fn capability(value: &str) -> CapabilityId {
        CapabilityId::new(value).unwrap()
    }

    fn version() -> Version {
        Version::new(1, 0, 0)
    }

    fn requirement() -> VersionReq {
        VersionReq::parse("^1.0").unwrap()
    }

    #[test]
    fn resolves_dependency_order_and_reverse_teardown() {
        let clock = capability("dev.kernox.clock");
        let provider = PluginDescriptor::new(plugin("dev.example.clock"), version())
            .provide(CapabilityOffer::new(clock.clone(), version()))
            .unwrap();
        let consumer = PluginDescriptor::new(plugin("dev.example.orders"), version())
            .require(CapabilityRequirement::exactly_one(clock, requirement()))
            .unwrap();

        let graph = GraphBuilder::new().plugin(consumer).plugin(provider).resolve().unwrap();
        let startup: Vec<_> = graph.startup_order().iter().map(PluginId::as_str).collect();
        let teardown: Vec<_> = graph.teardown_order().map(PluginId::as_str).collect();

        assert_eq!(startup, ["dev.example.clock", "dev.example.orders"]);
        assert_eq!(teardown, ["dev.example.orders", "dev.example.clock"]);
    }

    #[test]
    fn rejects_ambiguous_single_provider_and_accepts_binding() {
        let clock = capability("dev.kernox.clock");
        let first = PluginDescriptor::new(plugin("dev.example.clock-a"), version())
            .provide(CapabilityOffer::new(clock.clone(), version()))
            .unwrap();
        let second = PluginDescriptor::new(plugin("dev.example.clock-b"), version())
            .provide(CapabilityOffer::new(clock.clone(), version()))
            .unwrap();
        let consumer = PluginDescriptor::new(plugin("dev.example.orders"), version())
            .require(CapabilityRequirement::exactly_one(clock.clone(), requirement()))
            .unwrap();

        let error = GraphBuilder::new()
            .plugin(first.clone())
            .plugin(second.clone())
            .plugin(consumer.clone())
            .resolve()
            .unwrap_err();
        assert_eq!(error.tag(), "graph.ambiguous-provider");

        let graph = GraphBuilder::new()
            .plugin(first)
            .plugin(second)
            .plugin(consumer)
            .binding(Binding::new(
                plugin("dev.example.orders"),
                clock,
                plugin("dev.example.clock-b"),
            ))
            .resolve()
            .unwrap();
        assert_eq!(graph.requirements()[0].providers, [plugin("dev.example.clock-b")]);
    }

    #[test]
    fn returns_an_exact_cycle_path() {
        let cap_a = capability("dev.example.cap-a");
        let cap_b = capability("dev.example.cap-b");
        let a = PluginDescriptor::new(plugin("dev.example.a"), version())
            .provide(CapabilityOffer::new(cap_a.clone(), version()))
            .unwrap()
            .require(CapabilityRequirement::exactly_one(cap_b.clone(), requirement()))
            .unwrap();
        let b = PluginDescriptor::new(plugin("dev.example.b"), version())
            .provide(CapabilityOffer::new(cap_b, version()))
            .unwrap()
            .require(CapabilityRequirement::exactly_one(cap_a, requirement()))
            .unwrap();

        let error = GraphBuilder::new().plugin(a).plugin(b).resolve().unwrap_err();
        let ResolveError::DependencyCycle { cycle } = error else {
            panic!("expected dependency cycle");
        };
        assert_eq!(cycle.first(), cycle.last());
        assert_eq!(cycle.len(), 3);
    }

    #[test]
    fn rejects_an_unsupported_composition_schema() {
        let spec = CompositionSpec { schema_version: 2, ..CompositionSpec::default() };
        let error = GraphBuilder::from_spec(spec).resolve().unwrap_err();

        assert_eq!(error.tag(), "graph.unsupported-schema-version");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn rejects_unknown_composition_fields() {
        let input = r#"{
            "schema_version": 1,
            "limits": {
                "max_plugins": 1,
                "max_capabilities_per_plugin": 1,
                "max_edges": 1
            },
            "plugins": [],
            "bindings": [],
            "surprise": true
        }"#;

        let error = serde_json::from_str::<CompositionSpec>(input).unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }

    proptest! {
        #[test]
        fn independent_plugin_order_is_insertion_invariant(values in prop::collection::btree_set(0_u16..500, 1..64)) {
            let descriptors: Vec<_> = values
                .into_iter()
                .map(|value| {
                    PluginDescriptor::new(
                        plugin(&format!("dev.example.p{value}")),
                        version(),
                    )
                })
                .collect();

            let forward = descriptors
                .iter()
                .cloned()
                .fold(GraphBuilder::new(), GraphBuilder::plugin)
                .resolve()
                .unwrap();
            let reverse = descriptors
                .iter()
                .rev()
                .cloned()
                .fold(GraphBuilder::new(), GraphBuilder::plugin)
                .resolve()
                .unwrap();

            prop_assert_eq!(forward.report(), reverse.report());
        }
    }
}
