//! Host negotiation contract tests for duplicate host properties.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use futures::executor::block_on;
use kernox_core::{CapabilityId, PluginDescriptor, PluginId};
use kernox_runtime::{
    AppBuilder, AppResolveError, BoxFuture, HostCapability, HostRequirement, HostResolutionError,
    InitializationContext, Plugin, PluginError, ProvisionSet,
};
use semver::{Version, VersionReq};

/// Records how often the plugin lifecycle reached initialization.
struct InitializeProbe {
    descriptor: PluginDescriptor,
    host_requirements: Vec<HostRequirement>,
    initialized: Arc<AtomicUsize>,
}

impl InitializeProbe {
    fn new(plugin: PluginId, host_requirements: Vec<HostRequirement>) -> Self {
        Self {
            descriptor: PluginDescriptor::new(plugin, version(1, 0, 0)),
            host_requirements,
            initialized: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Plugin for InitializeProbe {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn host_requirements(&self) -> Vec<HostRequirement> {
        self.host_requirements.clone()
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        self.initialized.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(ProvisionSet::new()) })
    }
}

#[test]
fn duplicate_host_capability_fails_before_initialization() {
    block_on(async {
        let capability = capability_id("dev.example.host.available");
        let requirement =
            HostRequirement::new(capability.clone(), VersionReq::parse("^1.0").unwrap());

        // Control: one supplied capability satisfies the requirement and the
        // probe plugin reaches initialization.
        let accepted =
            InitializeProbe::new(plugin_id("dev.example.probe"), vec![requirement.clone()]);
        let mut app = AppBuilder::new()
            .host_capability(HostCapability::new(capability.clone(), version(1, 0, 0)))
            .plugin(accepted)
            .resolve()
            .expect("a single host capability satisfies the requirement")
            .start()
            .await
            .expect("a satisfied host contract must start");
        assert!(app.shutdown().await.is_clean());

        // A host that supplies the same identity twice is invalid regardless
        // of the supplied versions.
        let duplicated = InitializeProbe::new(plugin_id("dev.example.probe"), vec![requirement]);
        let initialized = Arc::clone(&duplicated.initialized);
        let error = AppBuilder::new()
            .host_capability(HostCapability::new(capability.clone(), version(1, 0, 0)))
            .host_capability(HostCapability::new(capability.clone(), version(2, 0, 0)))
            .plugin(duplicated)
            .resolve()
            .err()
            .expect("a duplicated host capability must fail closed");

        assert_eq!(error.tag(), "host.duplicate-capability");
        assert_eq!(
            error,
            AppResolveError::Host(HostResolutionError::DuplicateCapability {
                capability: capability.clone(),
            })
        );
        assert_eq!(initialized.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn duplicate_host_requirement_fails_before_initialization() {
    block_on(async {
        let capability = capability_id("dev.example.host.available");
        let requirement =
            HostRequirement::new(capability.clone(), VersionReq::parse("^1.0").unwrap());

        // Control: the same plugin with one declaration starts normally.
        let accepted =
            InitializeProbe::new(plugin_id("dev.example.probe"), vec![requirement.clone()]);
        let mut app = AppBuilder::new()
            .host_capability(HostCapability::new(capability.clone(), version(1, 0, 0)))
            .plugin(accepted)
            .resolve()
            .expect("a single host requirement satisfies negotiation")
            .start()
            .await
            .expect("a satisfied host contract must start");
        assert!(app.shutdown().await.is_clean());

        let plugin = plugin_id("dev.example.probe");
        let duplicated =
            InitializeProbe::new(plugin.clone(), vec![requirement.clone(), requirement]);
        let initialized = Arc::clone(&duplicated.initialized);
        let error = AppBuilder::new()
            .host_capability(HostCapability::new(capability.clone(), version(1, 0, 0)))
            .plugin(duplicated)
            .resolve()
            .err()
            .expect("a duplicated host requirement must fail closed");

        assert_eq!(error.tag(), "host.duplicate-requirement");
        assert_eq!(
            error,
            AppResolveError::Host(HostResolutionError::DuplicateRequirement { plugin, capability })
        );
        assert_eq!(initialized.load(Ordering::SeqCst), 0);
    });
}

fn plugin_id(value: &str) -> PluginId {
    PluginId::new(value).unwrap()
}

fn capability_id(value: &str) -> CapabilityId {
    CapabilityId::new(value).unwrap()
}

const fn version(major: u64, minor: u64, patch: u64) -> Version {
    Version::new(major, minor, patch)
}
