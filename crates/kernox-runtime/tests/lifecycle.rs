//! End-to-end lifecycle, provisioning, and rollback contract tests.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use futures::executor::block_on;
use kernox_core::{
    CapabilityId, CapabilityOffer, CapabilityRequirement, PluginDescriptor, PluginId,
};
use kernox_runtime::{
    AppBuilder, BoxFuture, Capability, InitializationContext, LifecycleContext,
    LifecycleObservation, ObservationSink, Plugin, PluginError, ProvisionSet, ScopeState,
};
use semver::{Version, VersionReq};

trait Clock: Send + Sync {
    fn tick(&self) -> u64;
}

struct ClockCapability;

impl Capability for ClockCapability {
    type Interface = dyn Clock;

    const ID: &'static str = "dev.example.clock";
    const VERSION: &'static str = "1.0.0";
}

struct OtherClockCapability;

impl Capability for OtherClockCapability {
    type Interface = dyn Clock;

    const ID: &'static str = "dev.example.clock";
    const VERSION: &'static str = "1.0.0";
}

struct FixedClock;

impl Clock for FixedClock {
    fn tick(&self) -> u64 {
        42
    }
}

struct DescriptorReadProbe {
    descriptor: PluginDescriptor,
    reads: Arc<AtomicUsize>,
}

impl Plugin for DescriptorReadProbe {
    fn descriptor(&self) -> &PluginDescriptor {
        self.reads.fetch_add(1, Ordering::Relaxed);
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        Box::pin(async { Ok(ProvisionSet::new()) })
    }
}

struct ClockPlugin {
    descriptor: PluginDescriptor,
    events: Arc<Mutex<Vec<String>>>,
    publish: bool,
    fail_stop: bool,
    scope_states: Option<Arc<Mutex<Vec<ScopeState>>>>,
}

impl ClockPlugin {
    fn new(events: Arc<Mutex<Vec<String>>>) -> Self {
        let descriptor = PluginDescriptor::new(plugin_id("dev.example.clock-plugin"), version())
            .provide(CapabilityOffer::new(capability_id(ClockCapability::ID), version()))
            .unwrap();
        Self { descriptor, events, publish: true, fail_stop: false, scope_states: None }
    }
}

impl Plugin for ClockPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        record(&self.events, "clock.initialize");
        let publish = self.publish;
        Box::pin(async move {
            if publish {
                let clock: Arc<dyn Clock> = Arc::new(FixedClock);
                ProvisionSet::new()
                    .provide::<ClockCapability>(clock)
                    .map_err(|error| PluginError::new(error.tag(), error.to_string()))
            } else {
                Ok(ProvisionSet::new())
            }
        })
    }

    fn start<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        record(&self.events, "clock.start");
        Box::pin(async { Ok(()) })
    }

    fn quiesce<'a>(
        &'a mut self,
        context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        record_scope_state(self.scope_states.as_ref(), context);
        record(&self.events, "clock.quiesce");
        Box::pin(async { Ok(()) })
    }

    fn stop<'a>(
        &'a mut self,
        context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        record_scope_state(self.scope_states.as_ref(), context);
        record(&self.events, "clock.stop");
        let fail = self.fail_stop;
        Box::pin(async move {
            if fail {
                Err(PluginError::new("clock.stop-failed", "injected stop failure"))
            } else {
                Ok(())
            }
        })
    }

    fn dispose<'a>(
        &'a mut self,
        context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        record_scope_state(self.scope_states.as_ref(), context);
        record(&self.events, "clock.dispose");
        Box::pin(async { Ok(()) })
    }
}

struct ConsumerPlugin {
    descriptor: PluginDescriptor,
    events: Arc<Mutex<Vec<String>>>,
    clock: Option<Arc<dyn Clock>>,
    fail_start: bool,
    use_wrong_marker: bool,
}

#[derive(Clone, Copy)]
enum CardinalityAccessMode {
    Optional,
    All,
}

struct CardinalityAccessPlugin {
    descriptor: PluginDescriptor,
    mode: CardinalityAccessMode,
}

impl CardinalityAccessPlugin {
    fn new(id: &str, mode: CardinalityAccessMode) -> Self {
        let descriptor = PluginDescriptor::new(plugin_id(id), version())
            .require(CapabilityRequirement::exactly_one(
                capability_id(ClockCapability::ID),
                VersionReq::parse("^1.0").unwrap(),
            ))
            .unwrap();
        Self { descriptor, mode }
    }
}

impl Plugin for CardinalityAccessPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let result = match self.mode {
            CardinalityAccessMode::Optional => context.optional::<ClockCapability>().map(|_| ()),
            CardinalityAccessMode::All => context.all::<ClockCapability>().map(|_| ()),
        };
        let result = result.map_err(|error| PluginError::new(error.tag(), error.to_string()));
        Box::pin(async move { result.map(|()| ProvisionSet::new()) })
    }
}

impl ConsumerPlugin {
    fn new(events: Arc<Mutex<Vec<String>>>) -> Self {
        let descriptor = PluginDescriptor::new(plugin_id("dev.example.consumer"), version())
            .require(CapabilityRequirement::exactly_one(
                capability_id(ClockCapability::ID),
                VersionReq::parse("^1.0").unwrap(),
            ))
            .unwrap();
        Self { descriptor, events, clock: None, fail_start: false, use_wrong_marker: false }
    }
}

impl Plugin for ConsumerPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        record(&self.events, "consumer.initialize");
        let result = if self.use_wrong_marker {
            context.require::<OtherClockCapability>()
        } else {
            context.require::<ClockCapability>()
        };
        match result {
            Ok(clock) => {
                self.clock = Some(clock);
                Box::pin(async { Ok(ProvisionSet::new()) })
            }
            Err(error) => {
                let failure = PluginError::new(error.tag(), error.to_string());
                Box::pin(async move { Err(failure) })
            }
        }
    }

    fn start<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        record(&self.events, "consumer.start");
        let fail = self.fail_start;
        Box::pin(async move {
            if fail {
                Err(PluginError::new("consumer.start-failed", "injected start failure"))
            } else {
                Ok(())
            }
        })
    }

    fn quiesce<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        record(&self.events, "consumer.quiesce");
        Box::pin(async { Ok(()) })
    }

    fn stop<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        record(&self.events, "consumer.stop");
        Box::pin(async { Ok(()) })
    }

    fn dispose<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        record(&self.events, "consumer.dispose");
        self.clock = None;
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn boots_with_direct_typed_handle_and_shuts_down_in_reverse_order_once() {
    block_on(async {
        let events = Arc::new(Mutex::new(Vec::new()));
        let resolved = AppBuilder::new()
            .plugin(ConsumerPlugin::new(Arc::clone(&events)))
            .plugin(ClockPlugin::new(Arc::clone(&events)))
            .resolve()
            .unwrap();
        let mut app = resolved.start().await.unwrap();
        let clock =
            app.capability_from::<ClockCapability>(&plugin_id("dev.example.clock-plugin")).unwrap();

        assert_eq!(clock.tick(), 42);
        let first = app.shutdown().await;
        let second = app.shutdown().await;
        assert!(first.is_clean());
        assert_eq!(first, second);
        assert_eq!(
            app.capability_from::<ClockCapability>(&plugin_id("dev.example.clock-plugin"))
                .err()
                .expect("root lookup must close after shutdown")
                .tag(),
            "access.application-unavailable"
        );
        assert_eq!(
            snapshot(&events),
            [
                "clock.initialize",
                "consumer.initialize",
                "clock.start",
                "consumer.start",
                "consumer.quiesce",
                "clock.quiesce",
                "consumer.stop",
                "clock.stop",
                "consumer.dispose",
                "clock.dispose",
            ]
        );
    });
}

#[test]
fn descriptor_is_snapshotted_once_before_resolution() {
    block_on(async {
        let reads = Arc::new(AtomicUsize::new(0));
        let probe = DescriptorReadProbe {
            descriptor: PluginDescriptor::new(plugin_id("dev.example.probe"), version()),
            reads: Arc::clone(&reads),
        };
        let mut app = AppBuilder::new().plugin(probe).resolve().unwrap().start().await.unwrap();

        assert_eq!(reads.load(Ordering::Relaxed), 1);
        assert!(app.shutdown().await.is_clean());
        assert_eq!(reads.load(Ordering::Relaxed), 1);
    });
}

#[test]
fn missing_provision_is_not_committed_and_current_plugin_is_disposed() {
    block_on(async {
        let events = Arc::new(Mutex::new(Vec::new()));
        let scope_states = Arc::new(Mutex::new(Vec::new()));
        let mut clock = ClockPlugin::new(Arc::clone(&events));
        clock.publish = false;
        clock.scope_states = Some(Arc::clone(&scope_states));
        let error = AppBuilder::new()
            .plugin(clock)
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .expect("boot must fail");

        assert_eq!(error.primary.error_tag, "provision.missing");
        assert!(error.cleanup_failures.is_empty());
        assert_eq!(snapshot(&events), ["clock.initialize", "clock.dispose"]);
        assert_eq!(snapshot_scope_states(&scope_states), [ScopeState::Closing]);
    });
}

#[test]
fn undeclared_and_version_mismatched_provisions_fail_before_readiness() {
    block_on(async {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut undeclared = ClockPlugin::new(Arc::clone(&events));
        undeclared.descriptor =
            PluginDescriptor::new(plugin_id("dev.example.clock-plugin"), version());
        let error = AppBuilder::new()
            .plugin(undeclared)
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .expect("undeclared provision must fail");
        assert_eq!(error.primary.error_tag, "provision.undeclared");

        let mut mismatched = ClockPlugin::new(events);
        mismatched.descriptor =
            PluginDescriptor::new(plugin_id("dev.example.clock-plugin"), version())
                .provide(CapabilityOffer::new(
                    capability_id(ClockCapability::ID),
                    Version::new(2, 0, 0),
                ))
                .unwrap();
        let error = AppBuilder::new()
            .plugin(mismatched)
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .expect("version mismatch must fail");
        assert_eq!(error.primary.error_tag, "provision.version-mismatch");
    });
}

#[test]
fn duplicate_staged_provision_is_rejected_locally() {
    let first: Arc<dyn Clock> = Arc::new(FixedClock);
    let second: Arc<dyn Clock> = Arc::new(FixedClock);
    let staged = ProvisionSet::new().provide::<ClockCapability>(first).unwrap();

    assert_eq!(
        staged
            .provide::<ClockCapability>(second)
            .err()
            .expect("duplicate provision must fail")
            .tag(),
        "provision.duplicate"
    );
}

#[test]
fn start_failure_rolls_back_all_phases_and_preserves_cleanup_failures() {
    block_on(async {
        let events = Arc::new(Mutex::new(Vec::new()));
        let scope_states = Arc::new(Mutex::new(Vec::new()));
        let mut clock = ClockPlugin::new(Arc::clone(&events));
        clock.fail_stop = true;
        clock.scope_states = Some(Arc::clone(&scope_states));
        let mut consumer = ConsumerPlugin::new(Arc::clone(&events));
        consumer.fail_start = true;

        let error = AppBuilder::new()
            .plugin(consumer)
            .plugin(clock)
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .expect("boot must fail");

        assert_eq!(error.primary.error_tag, "consumer.start-failed");
        assert_eq!(error.cleanup_failures.len(), 1);
        assert_eq!(error.cleanup_failures[0].error_tag, "clock.stop-failed");
        assert_eq!(
            snapshot(&events),
            [
                "clock.initialize",
                "consumer.initialize",
                "clock.start",
                "consumer.start",
                "consumer.quiesce",
                "clock.quiesce",
                "consumer.stop",
                "clock.stop",
                "consumer.dispose",
                "clock.dispose",
            ]
        );
        assert_eq!(
            snapshot_scope_states(&scope_states),
            [ScopeState::Closing, ScopeState::Closing, ScopeState::Closing]
        );
    });
}

#[test]
fn same_identity_with_a_different_marker_type_fails_closed() {
    block_on(async {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut consumer = ConsumerPlugin::new(Arc::clone(&events));
        consumer.use_wrong_marker = true;

        let error = AppBuilder::new()
            .plugin(consumer)
            .plugin(ClockPlugin::new(events))
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .expect("boot must fail");

        assert_eq!(error.primary.error_tag, "access.type-mismatch");
    });
}

#[test]
fn undeclared_capability_access_fails_closed() {
    block_on(async {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut consumer = ConsumerPlugin::new(Arc::clone(&events));
        consumer.descriptor = PluginDescriptor::new(plugin_id("dev.example.consumer"), version());

        let error = AppBuilder::new()
            .plugin(consumer)
            .plugin(ClockPlugin::new(events))
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .expect("boot must fail");

        assert_eq!(error.primary.error_tag, "access.undeclared");
    });
}

#[test]
fn dependency_access_mode_must_match_declared_cardinality() {
    block_on(async {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut consumer = ConsumerPlugin::new(Arc::clone(&events));
        consumer.descriptor = PluginDescriptor::new(plugin_id("dev.example.consumer"), version())
            .require(CapabilityRequirement::new(
                capability_id(ClockCapability::ID),
                VersionReq::parse("^1.0").unwrap(),
                kernox_core::RequirementCardinality::ZeroOrOne,
            ))
            .unwrap();

        let error = AppBuilder::new()
            .plugin(consumer)
            .plugin(ClockPlugin::new(events))
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .expect("wrong access mode must fail");

        assert_eq!(error.primary.error_tag, "access.cardinality-mismatch");
    });
}

#[test]
fn optional_and_all_reject_single_provider_requirements() {
    block_on(async {
        for (id, mode) in [
            ("dev.example.optional-access", CardinalityAccessMode::Optional),
            ("dev.example.all-access", CardinalityAccessMode::All),
        ] {
            let error = AppBuilder::new()
                .plugin(CardinalityAccessPlugin::new(id, mode))
                .plugin(ClockPlugin::new(Arc::new(Mutex::new(Vec::new()))))
                .resolve()
                .unwrap()
                .start()
                .await
                .err()
                .expect("the access mode must fail closed");

            assert_eq!(error.primary.error_tag, "access.cardinality-mismatch");
        }
    });
}

#[test]
fn concurrent_invocations_receive_unique_scopes_with_the_app_as_parent() {
    block_on(async {
        let mut app = AppBuilder::new().resolve().unwrap().start().await.unwrap();
        let app_scope = app.scope().id();
        let identities = Arc::new(Mutex::new(Vec::new()));

        std::thread::scope(|threads| {
            for _ in 0..64 {
                let identities = Arc::clone(&identities);
                let app_ref = &app;
                threads.spawn(move || {
                    let invocation = app_ref.invocation_scope().unwrap();
                    assert_eq!(invocation.view().parent(), Some(app_scope));
                    identities.lock().unwrap().push(invocation.view().id());
                });
            }
        });

        {
            let identities = identities.lock().unwrap();
            let unique: std::collections::BTreeSet<_> = identities.iter().copied().collect();
            assert_eq!(identities.len(), 64);
            assert_eq!(unique.len(), 64);
        }
        assert!(app.shutdown().await.is_clean());
    });
}

#[derive(Clone, Copy)]
enum PanicHook {
    InitializeSync,
    InitializeAsync,
    Start,
    Dispose,
}

struct PanicPlugin {
    descriptor: PluginDescriptor,
    events: Arc<Mutex<Vec<String>>>,
    hook: PanicHook,
}

impl PanicPlugin {
    fn new(id: &str, events: Arc<Mutex<Vec<String>>>, hook: PanicHook) -> Self {
        Self { descriptor: PluginDescriptor::new(plugin_id(id), version()), events, hook }
    }
}

impl Plugin for PanicPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        record(&self.events, "panic.initialize");
        match self.hook {
            PanicHook::InitializeSync => panic!("injected initialize panic"),
            PanicHook::InitializeAsync => Box::pin(async { panic!("injected initialize panic") }),
            PanicHook::Start | PanicHook::Dispose => Box::pin(async { Ok(ProvisionSet::new()) }),
        }
    }

    fn start<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        record(&self.events, "panic.start");
        if matches!(self.hook, PanicHook::Start) {
            Box::pin(async { panic!("injected start panic") })
        } else {
            Box::pin(async { Ok(()) })
        }
    }

    fn dispose<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        record(&self.events, "panic.dispose");
        if matches!(self.hook, PanicHook::Dispose) {
            Box::pin(async { panic!("injected dispose panic") })
        } else {
            Box::pin(async { Ok(()) })
        }
    }
}

struct PanicSink;

impl ObservationSink for PanicSink {
    fn record(&self, _observation: LifecycleObservation) {
        panic!("injected observation sink panic");
    }
}

#[test]
fn initialize_hook_unwind_rolls_back_already_initialized_plugins() {
    block_on(async {
        let events = Arc::new(Mutex::new(Vec::new()));
        let error = AppBuilder::new()
            .plugin(ClockPlugin::new(Arc::clone(&events)))
            .plugin(PanicPlugin::new(
                "dev.example.panic-plugin",
                Arc::clone(&events),
                PanicHook::InitializeAsync,
            ))
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .expect("initialize unwind must become a lifecycle failure");

        assert_eq!(error.primary.error_tag, "plugin.hook-panicked");
        assert!(error.cleanup_failures.is_empty());
        assert_eq!(
            snapshot(&events),
            ["clock.initialize", "panic.initialize", "panic.dispose", "clock.dispose"]
        );
    });
}

#[test]
fn initialize_builder_unwind_is_isolated_like_hook_unwind() {
    block_on(async {
        let events = Arc::new(Mutex::new(Vec::new()));
        let error = AppBuilder::new()
            .plugin(ClockPlugin::new(Arc::clone(&events)))
            .plugin(PanicPlugin::new(
                "dev.example.panic-plugin",
                Arc::clone(&events),
                PanicHook::InitializeSync,
            ))
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .expect("sync initialize panic must become a lifecycle failure");

        assert_eq!(error.primary.error_tag, "plugin.hook-panicked");
        assert_eq!(
            snapshot(&events),
            ["clock.initialize", "panic.initialize", "panic.dispose", "clock.dispose"]
        );
    });
}

#[test]
fn start_hook_unwind_runs_reverse_cleanup() {
    block_on(async {
        let events = Arc::new(Mutex::new(Vec::new()));
        let error = AppBuilder::new()
            .plugin(ClockPlugin::new(Arc::clone(&events)))
            .plugin(PanicPlugin::new(
                "dev.example.panic-plugin",
                Arc::clone(&events),
                PanicHook::Start,
            ))
            .resolve()
            .unwrap()
            .start()
            .await
            .err()
            .expect("start unwind must become a lifecycle failure");

        assert_eq!(error.primary.error_tag, "plugin.hook-panicked");
        assert_eq!(
            snapshot(&events),
            [
                "clock.initialize",
                "panic.initialize",
                "clock.start",
                "panic.start",
                "clock.quiesce",
                "clock.stop",
                "panic.dispose",
                "clock.dispose",
            ]
        );
    });
}

#[test]
fn cleanup_hook_unwind_continues_later_hooks() {
    block_on(async {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut app = AppBuilder::new()
            .plugin(ClockPlugin::new(Arc::clone(&events)))
            .plugin(PanicPlugin::new(
                "dev.example.panic-plugin",
                Arc::clone(&events),
                PanicHook::Dispose,
            ))
            .resolve()
            .unwrap()
            .start()
            .await
            .unwrap();

        let report = app.shutdown().await;
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].error_tag, "plugin.hook-panicked");
        assert_eq!(
            snapshot(&events),
            [
                "clock.initialize",
                "panic.initialize",
                "clock.start",
                "panic.start",
                "clock.quiesce",
                "clock.stop",
                "panic.dispose",
                "clock.dispose",
            ]
        );
    });
}

#[test]
fn observation_sink_unwind_does_not_abort_lifecycle() {
    block_on(async {
        let mut app = AppBuilder::new()
            .plugin(ClockPlugin::new(Arc::new(Mutex::new(Vec::new()))))
            .observation_sink(Arc::new(PanicSink))
            .resolve()
            .unwrap()
            .start()
            .await
            .expect("a panicking sink must not abort startup");

        assert!(app.shutdown().await.is_clean());
    });
}

fn record(events: &Mutex<Vec<String>>, event: &str) {
    events.lock().unwrap().push(event.to_owned());
}

fn record_scope_state(states: Option<&Arc<Mutex<Vec<ScopeState>>>>, context: LifecycleContext<'_>) {
    if let Some(states) = states {
        states.lock().unwrap().push(context.scope().state());
    }
}

fn snapshot(events: &Mutex<Vec<String>>) -> Vec<String> {
    events.lock().unwrap().clone()
}

fn snapshot_scope_states(states: &Mutex<Vec<ScopeState>>) -> Vec<ScopeState> {
    states.lock().unwrap().clone()
}

fn plugin_id(value: &str) -> PluginId {
    PluginId::new(value).unwrap()
}

fn capability_id(value: &str) -> CapabilityId {
    CapabilityId::new(value).unwrap()
}

const fn version() -> Version {
    Version::new(1, 0, 0)
}
