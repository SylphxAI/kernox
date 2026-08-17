use std::{
    collections::BTreeMap, future::Future, marker::PhantomData, panic::AssertUnwindSafe, sync::Arc,
    time::Instant,
};

use futures_util::FutureExt;
use kernox_core::{Binding, GraphBuilder, GraphLimits, PluginId, ResolveError, ResolvedGraph};

use crate::capability::{Registry, root_capability};
use crate::observation::default_sink;
use crate::scope::Scope;
use crate::{
    AccessError, FailureRecord, InitializationContext, LifecycleContext, LifecycleFailure,
    LifecycleObservation, LifecycleOutcome, LifecyclePhase, ObservationSink, Plugin, PluginError,
    ScopeError, ScopeKind, ScopeView, ShutdownReport,
};

/// Composes native plugins and explicit provider choices into an application.
#[derive(Default)]
pub struct AppBuilder {
    plugins: Vec<Box<dyn Plugin>>,
    bindings: Vec<Binding>,
    limits: GraphLimits,
    observer: Option<Arc<dyn ObservationSink>>,
}

impl AppBuilder {
    /// Creates an empty application composition.
    #[must_use]
    pub fn new() -> Self {
        Self { limits: GraphLimits::default(), ..Self::default() }
    }

    /// Adds one statically linked plugin instance.
    #[must_use]
    pub fn plugin<P: Plugin>(mut self, plugin: P) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Selects a provider for one single-provider requirement.
    #[must_use]
    pub fn binding(mut self, binding: Binding) -> Self {
        self.bindings.push(binding);
        self
    }

    /// Replaces graph resource limits.
    #[must_use]
    pub const fn graph_limits(mut self, limits: GraphLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Installs a provider-neutral lifecycle observation sink.
    #[must_use]
    pub fn observation_sink(mut self, observer: Arc<dyn ObservationSink>) -> Self {
        self.observer = Some(observer);
        self
    }

    /// Resolves and validates the immutable capability graph without running I/O.
    ///
    /// # Errors
    ///
    /// Returns [`ResolveError`] when identities, dependencies, bindings,
    /// conflicts, cycles, or resource limits are invalid.
    pub fn resolve(self) -> Result<ResolvedApp, ResolveError> {
        let Self { plugins, bindings, limits, observer } = self;
        let declared: Vec<_> =
            plugins.into_iter().map(|plugin| (plugin.descriptor().clone(), plugin)).collect();
        let mut graph_builder = GraphBuilder::new().with_limits(limits);
        for (descriptor, _) in &declared {
            graph_builder = graph_builder.plugin(descriptor.clone());
        }
        for binding in bindings {
            graph_builder = graph_builder.binding(binding);
        }
        let graph = Arc::new(graph_builder.resolve()?);
        let plugins = declared
            .into_iter()
            .map(|(descriptor, plugin)| (descriptor.id().clone(), plugin))
            .collect();

        Ok(ResolvedApp {
            graph,
            plugins,
            observer: observer.unwrap_or_else(default_sink),
            scope: Scope::application(),
        })
    }
}

/// Validated application that has not performed plugin I/O.
#[must_use = "a resolved application must be started to make its capabilities ready"]
pub struct ResolvedApp {
    graph: Arc<ResolvedGraph>,
    plugins: BTreeMap<PluginId, Box<dyn Plugin>>,
    observer: Arc<dyn ObservationSink>,
    scope: Scope,
}

impl ResolvedApp {
    /// Returns the immutable resolved graph.
    #[must_use]
    pub fn graph(&self) -> &ResolvedGraph {
        &self.graph
    }

    /// Initializes all provisions transactionally, then starts every plugin.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleFailure`] after attempting complete reverse-order
    /// rollback when any initialization, validation, or start phase fails.
    pub async fn start(mut self) -> Result<RunningApp, LifecycleFailure> {
        let order = self.graph.startup_order().to_vec();
        let mut registry = Registry::default();
        let initialized = self.initialize_all(&order, &mut registry).await?;
        self.start_all(&order, &initialized).await?;

        Ok(RunningApp {
            graph: self.graph,
            plugins: self.plugins,
            registry,
            observer: self.observer,
            scope: self.scope,
            terminal: None,
        })
    }

    async fn initialize_all(
        &mut self,
        order: &[PluginId],
        registry: &mut Registry,
    ) -> Result<Vec<PluginId>, LifecycleFailure> {
        let mut initialized = Vec::with_capacity(order.len());
        for plugin_id in order {
            self.initialize_one(plugin_id, registry, &initialized).await?;
            initialized.push(plugin_id.clone());
        }
        Ok(initialized)
    }

    async fn initialize_one(
        &mut self,
        plugin_id: &PluginId,
        registry: &mut Registry,
        initialized: &[PluginId],
    ) -> Result<(), LifecycleFailure> {
        let started_at = Instant::now();
        let graph = Arc::clone(&self.graph);
        let scope = self.scope.clone();
        let result = {
            let plugin = self.plugin_mut(plugin_id);
            run_plugin_hook(|| {
                plugin.initialize(InitializationContext::new(
                    plugin_id,
                    &graph,
                    registry,
                    scope.view(),
                ))
            })
            .await
        };

        let staged = match result {
            Ok(staged) => staged,
            Err(error) => {
                return Err(self
                    .rollback_initialize(
                        plugin_id,
                        started_at,
                        safe_tag(error.tag()),
                        plugin_failure(plugin_id, LifecyclePhase::Initialize, &error),
                        initialized,
                    )
                    .await);
            }
        };

        let descriptor = self
            .graph
            .plugin(plugin_id)
            .unwrap_or_else(|| unreachable!("startup order contains only resolved plugins"));
        if let Err(error) = registry.commit(plugin_id, descriptor, staged) {
            return Err(self
                .rollback_initialize(
                    plugin_id,
                    started_at,
                    error.tag(),
                    FailureRecord {
                        phase: LifecyclePhase::Initialize,
                        plugin: plugin_id.clone(),
                        error_tag: error.tag(),
                        message: error.to_string(),
                    },
                    initialized,
                )
                .await);
        }

        self.observe_success(plugin_id, LifecyclePhase::Initialize, started_at);
        Ok(())
    }

    async fn rollback_initialize(
        &mut self,
        plugin_id: &PluginId,
        started_at: Instant,
        error_tag: &'static str,
        primary: FailureRecord,
        initialized: &[PluginId],
    ) -> LifecycleFailure {
        self.observe_failure(plugin_id, LifecyclePhase::Initialize, started_at, error_tag);
        let mut rollback = initialized.to_vec();
        rollback.push(plugin_id.clone());
        let cleanup_failures = self.cleanup(&rollback, &[LifecyclePhase::Dispose]).await;
        self.close_scope();
        LifecycleFailure { primary, cleanup_failures }
    }

    async fn start_all(
        &mut self,
        order: &[PluginId],
        initialized: &[PluginId],
    ) -> Result<(), LifecycleFailure> {
        let mut started = Vec::with_capacity(order.len());
        for plugin_id in order {
            self.start_one(plugin_id, &started, initialized).await?;
            started.push(plugin_id.clone());
        }
        Ok(())
    }

    async fn start_one(
        &mut self,
        plugin_id: &PluginId,
        started: &[PluginId],
        initialized: &[PluginId],
    ) -> Result<(), LifecycleFailure> {
        let started_at = Instant::now();
        let scope = self.scope.clone();
        let result = {
            let plugin = self.plugin_mut(plugin_id);
            run_plugin_hook(|| plugin.start(LifecycleContext::new(scope.view()))).await
        };
        match &result {
            Ok(()) => self.observe_success(plugin_id, LifecyclePhase::Start, started_at),
            Err(error) => self.observe_failure(
                plugin_id,
                LifecyclePhase::Start,
                started_at,
                safe_tag(error.tag()),
            ),
        }
        if let Err(error) = result {
            let primary = plugin_failure(plugin_id, LifecyclePhase::Start, &error);
            let mut affected = started.to_vec();
            affected.push(plugin_id.clone());
            let mut cleanup_failures =
                self.cleanup(&affected, &[LifecyclePhase::Quiesce, LifecyclePhase::Stop]).await;
            cleanup_failures.extend(self.cleanup(initialized, &[LifecyclePhase::Dispose]).await);
            self.close_scope();
            return Err(LifecycleFailure { primary, cleanup_failures });
        }
        Ok(())
    }

    fn plugin_mut(&mut self, plugin: &PluginId) -> &mut Box<dyn Plugin> {
        self.plugins
            .get_mut(plugin)
            .unwrap_or_else(|| unreachable!("resolved graph and plugin map are built together"))
    }

    async fn cleanup(
        &mut self,
        plugins: &[PluginId],
        phases: &[LifecyclePhase],
    ) -> Vec<FailureRecord> {
        let mut failures = Vec::new();
        for phase in phases {
            for plugin_id in plugins.iter().rev() {
                if let Err(failure) = self.run_cleanup(plugin_id, *phase).await {
                    failures.push(failure);
                }
            }
        }
        failures
    }

    async fn run_cleanup(
        &mut self,
        plugin_id: &PluginId,
        phase: LifecyclePhase,
    ) -> Result<(), FailureRecord> {
        let scope = self.scope.clone();
        let observer = Arc::clone(&self.observer);
        run_cleanup_hook(
            self.plugin_mut(plugin_id).as_mut(),
            plugin_id,
            phase,
            scope.view(),
            &observer,
        )
        .await
    }

    fn observe_success(&self, plugin: &PluginId, phase: LifecyclePhase, started_at: Instant) {
        emit_observation(
            self.observer.as_ref(),
            LifecycleObservation {
                plugin: plugin.clone(),
                scope: self.scope.view().id(),
                phase,
                outcome: LifecycleOutcome::Succeeded,
                duration: started_at.elapsed(),
            },
        );
    }

    fn observe_failure(
        &self,
        plugin: &PluginId,
        phase: LifecyclePhase,
        started_at: Instant,
        error_tag: &'static str,
    ) {
        emit_observation(
            self.observer.as_ref(),
            LifecycleObservation {
                plugin: plugin.clone(),
                scope: self.scope.view().id(),
                phase,
                outcome: LifecycleOutcome::Failed { error_tag },
                duration: started_at.elapsed(),
            },
        );
    }

    fn close_scope(&self) {
        self.scope.begin_close();
        self.scope.finish_close();
    }
}

/// Ready application with immutable provisions and direct typed exports.
#[must_use = "a running application should be shut down to execute plugin cleanup hooks"]
pub struct RunningApp {
    graph: Arc<ResolvedGraph>,
    plugins: BTreeMap<PluginId, Box<dyn Plugin>>,
    registry: Registry,
    observer: Arc<dyn ObservationSink>,
    scope: Scope,
    terminal: Option<ShutdownReport>,
}

impl RunningApp {
    /// Returns the immutable graph used to boot this exact application.
    #[must_use]
    pub fn graph(&self) -> &ResolvedGraph {
        &self.graph
    }

    /// Returns a non-owning application scope view.
    #[must_use]
    pub fn scope(&self) -> ScopeView<'_> {
        self.scope.view()
    }

    /// Opens one fresh invocation scope tied to this application's borrow.
    ///
    /// # Errors
    ///
    /// Returns [`ScopeError`] after application shutdown admission has closed.
    pub fn invocation_scope(&self) -> Result<InvocationScope<'_>, ScopeError> {
        self.scope
            .child(ScopeKind::Invocation)
            .map(|scope| InvocationScope { scope, _app_borrow: PhantomData })
    }

    /// Acquires one explicit root export and returns a direct typed handle.
    ///
    /// This lookup is a composition-boundary operation. Keep the returned
    /// `Arc`; normal method calls on it do not access Kernox.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError`] when shutdown has begun, the provider did not
    /// publish the capability, or its marker type differs.
    pub fn capability_from<C: crate::Capability>(
        &self,
        provider: &PluginId,
    ) -> Result<Arc<C::Interface>, AccessError> {
        if self.terminal.is_some() || self.scope.view().is_closing() {
            return Err(AccessError::ApplicationUnavailable);
        }
        root_capability::<C>(&self.registry, provider)
    }

    /// Quiesces, stops, and disposes all plugins in reverse dependency order.
    ///
    /// Every hook is attempted after failures. Repeated calls return the exact
    /// cached terminal report and perform no lifecycle effect.
    pub async fn shutdown(&mut self) -> ShutdownReport {
        if let Some(report) = &self.terminal {
            return report.clone();
        }

        self.scope.begin_close();
        let order = self.graph.startup_order().to_vec();
        let mut failures = Vec::new();
        for phase in [LifecyclePhase::Quiesce, LifecyclePhase::Stop, LifecyclePhase::Dispose] {
            for plugin_id in order.iter().rev() {
                let result = run_cleanup_hook(
                    self.plugins
                        .get_mut(plugin_id)
                        .unwrap_or_else(|| unreachable!("running app preserves resolved plugins"))
                        .as_mut(),
                    plugin_id,
                    phase,
                    self.scope.view(),
                    &self.observer,
                )
                .await;
                if let Err(failure) = result {
                    failures.push(failure);
                }
            }
        }
        self.scope.finish_close();

        let report = ShutdownReport { failures };
        self.terminal = Some(report.clone());
        report
    }
}

/// Fresh request/invocation ownership boundary.
///
/// Dropping this guard closes the scope. Its lifetime prevents the supported
/// API from allowing invocation-local scope views to outlive the application.
///
/// ```compile_fail
/// use kernox_runtime::{InvocationScope, ScopeView};
///
/// fn leak(scope: &InvocationScope<'_>) -> ScopeView<'static> {
///     scope.view()
/// }
/// ```
#[must_use = "dropping the invocation scope closes its admission boundary"]
pub struct InvocationScope<'app> {
    scope: Scope,
    _app_borrow: PhantomData<&'app RunningApp>,
}

impl InvocationScope<'_> {
    /// Returns the non-owning invocation scope view.
    #[must_use]
    pub fn view(&self) -> ScopeView<'_> {
        self.scope.view()
    }

    /// Closes the invocation scope explicitly and consumes the guard.
    pub fn close(self) {}
}

impl Drop for InvocationScope<'_> {
    fn drop(&mut self) {
        self.scope.begin_close();
        self.scope.finish_close();
    }
}

async fn run_cleanup_hook(
    plugin: &mut dyn Plugin,
    plugin_id: &PluginId,
    phase: LifecyclePhase,
    scope: ScopeView<'_>,
    observer: &Arc<dyn ObservationSink>,
) -> Result<(), FailureRecord> {
    let started_at = Instant::now();
    let context = LifecycleContext::new(scope);
    let result = run_plugin_hook(|| match phase {
        LifecyclePhase::Quiesce => plugin.quiesce(context),
        LifecyclePhase::Stop => plugin.stop(context),
        LifecyclePhase::Dispose => plugin.dispose(context),
        LifecyclePhase::Initialize | LifecyclePhase::Start => {
            unreachable!("cleanup only invokes quiesce, stop, and dispose")
        }
    })
    .await;
    let outcome = match &result {
        Ok(()) => LifecycleOutcome::Succeeded,
        Err(error) => LifecycleOutcome::Failed { error_tag: safe_tag(error.tag()) },
    };
    emit_observation(
        observer.as_ref(),
        LifecycleObservation {
            plugin: plugin_id.clone(),
            scope: scope.id(),
            phase,
            outcome,
            duration: started_at.elapsed(),
        },
    );
    result.map_err(|error| plugin_failure(plugin_id, phase, &error))
}

const HOOK_PANICKED_TAG: &str = "plugin.hook-panicked";

fn hook_panicked() -> PluginError {
    PluginError::new(HOOK_PANICKED_TAG, "plugin lifecycle hook panicked")
}

async fn run_plugin_hook<T, Fut>(build: impl FnOnce() -> Fut) -> Result<T, PluginError>
where
    Fut: Future<Output = Result<T, PluginError>>,
{
    let Ok(future) = std::panic::catch_unwind(AssertUnwindSafe(build)) else {
        return Err(hook_panicked());
    };
    match AssertUnwindSafe(future).catch_unwind().await {
        Ok(result) => result,
        Err(_) => Err(hook_panicked()),
    }
}

fn emit_observation(observer: &dyn ObservationSink, observation: LifecycleObservation) {
    drop(std::panic::catch_unwind(AssertUnwindSafe(|| observer.record(observation))));
}

fn plugin_failure(plugin: &PluginId, phase: LifecyclePhase, error: &PluginError) -> FailureRecord {
    FailureRecord {
        phase,
        plugin: plugin.clone(),
        error_tag: safe_tag(error.tag()),
        message: error.message().to_owned(),
    }
}

fn safe_tag(tag: &'static str) -> &'static str {
    let valid = !tag.is_empty()
        && tag.len() <= 127
        && tag.bytes().all(|byte| byte.is_ascii_lowercase() || byte == b'.' || byte == b'-')
        && tag.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && tag.as_bytes().last().is_some_and(u8::is_ascii_lowercase);
    if valid { tag } else { "plugin.invalid-error-tag" }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::manual_let_else, clippy::panic, clippy::unwrap_used)]

    use super::*;
    use crate::Capability;

    trait Export: Send + Sync {}
    struct ExportCapability;

    impl Capability for ExportCapability {
        type Interface = dyn Export;

        const ID: &'static str = "dev.kernox.runtime.export";
        const VERSION: &'static str = "1.0.0";
    }

    #[test]
    fn root_capability_access_closes_when_shutdown_begins() {
        let app = RunningApp {
            graph: Arc::new(GraphBuilder::new().resolve().expect("empty graph resolves")),
            plugins: BTreeMap::new(),
            registry: Registry::default(),
            observer: default_sink(),
            scope: Scope::application(),
            terminal: None,
        };
        let provider = PluginId::new("dev.kernox.runtime.export-plugin").expect("valid provider");
        app.scope.begin_close();
        let error = match app.capability_from::<ExportCapability>(&provider) {
            Ok(_) => panic!("root capability access remained open after shutdown began"),
            Err(error) => error,
        };
        assert_eq!(error.tag(), "access.application-unavailable");
    }
}
