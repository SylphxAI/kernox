//! End-to-end lifecycle, provisioning, and rollback contract tests.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use futures::executor::block_on;
use kernox_core::{
    CapabilityId, CapabilityOffer, CapabilityRequirement, PluginDescriptor, PluginId,
};
use kernox_runtime::{
    AppBuilder, BoxFuture, Capability, InitializationContext, LifecycleContext, Plugin,
    PluginError, ProvisionSet,
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
}

impl ClockPlugin {
    fn new(events: Arc<Mutex<Vec<String>>>) -> Self {
        let descriptor = PluginDescriptor::new(plugin_id("dev.example.clock-plugin"), version())
            .provide(CapabilityOffer::new(capability_id(ClockCapability::ID), version()))
            .unwrap();
        Self { descriptor, events, publish: true, fail_stop: false }
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
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        record(&self.events, "clock.quiesce");
        Box::pin(async { Ok(()) })
    }

    fn stop<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
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
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
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
        let mut clock = ClockPlugin::new(Arc::clone(&events));
        clock.publish = false;
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
        let mut clock = ClockPlugin::new(Arc::clone(&events));
        clock.fail_stop = true;
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

fn record(events: &Mutex<Vec<String>>, event: &str) {
    events.lock().unwrap().push(event.to_owned());
}

fn snapshot(events: &Mutex<Vec<String>>) -> Vec<String> {
    events.lock().unwrap().clone()
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
