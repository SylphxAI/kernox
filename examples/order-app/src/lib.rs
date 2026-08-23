//! Host-neutral domain plugins used unchanged by both reference binaries.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use kernox::{
    AppBuilder, BoxFuture, Capability, CapabilityId, CapabilityOffer, CapabilityRequirement,
    InitializationContext, Plugin, PluginDescriptor, PluginError, PluginId, ProvisionSet,
    ResolvedApp,
};
use semver::{Version, VersionReq};
use thiserror::Error;

use kernox::core::PluginSource;

const VERSION: &str = "1.0.0";
const CLOCK_PLUGIN_ID: &str = "dev.kernox.examples.system-clock";
const STORE_PLUGIN_ID: &str = "dev.kernox.examples.order-store";
const SERVICE_PLUGIN_ID: &str = "dev.kernox.examples.order-service";
const CLOCK_CAPABILITY_ID: &str = "dev.kernox.examples.clock";
const STORE_CAPABILITY_ID: &str = "dev.kernox.examples.orders.store";
const SERVICE_CAPABILITY_ID: &str = "dev.kernox.examples.orders.service";

/// Failure to construct the compile-time reference composition.
#[derive(Debug, Error)]
pub enum ComposeError {
    /// A built-in stable identifier is invalid.
    #[error(transparent)]
    Identifier(#[from] kernox::core::IdentifierError),
    /// A descriptor declaration is internally inconsistent.
    #[error(transparent)]
    Descriptor(#[from] kernox::core::DescriptorError),
    /// A built-in semantic version is invalid.
    #[error(transparent)]
    Version(#[from] semver::Error),
    /// The selected capability graph is invalid.
    #[error(transparent)]
    Resolve(#[from] kernox::runtime::AppResolveError),
}

/// Example clock port.
pub trait Clock: Send + Sync {
    /// Returns milliseconds since the Unix epoch, saturating to zero before it.
    fn now_millis(&self) -> u128;
}

/// Marker for the clock port.
pub struct ClockCapability;

impl Capability for ClockCapability {
    type Interface = dyn Clock;

    const ID: &'static str = CLOCK_CAPABILITY_ID;
    const VERSION: &'static str = VERSION;
}

/// Persisted example order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Order {
    /// Monotonic application-local identifier.
    pub id: u64,
    /// Caller-provided stock-keeping unit.
    pub sku: String,
    /// Creation timestamp supplied through the clock port.
    pub created_at_millis: u128,
}

/// Example order persistence port.
pub trait OrderStore: Send + Sync {
    /// Saves one order.
    fn save(&self, order: Order);
    /// Loads one order by identity.
    fn find(&self, id: u64) -> Option<Order>;
}

/// Marker for the order persistence port.
pub struct OrderStoreCapability;

impl Capability for OrderStoreCapability {
    type Interface = dyn OrderStore;

    const ID: &'static str = STORE_CAPABILITY_ID;
    const VERSION: &'static str = VERSION;
}

/// Example application use-case port.
pub trait OrderService: Send + Sync {
    /// Creates and persists one order.
    fn create(&self, sku: String) -> Order;
}

/// Marker for the order application service.
pub struct OrderServiceCapability;

impl Capability for OrderServiceCapability {
    type Interface = dyn OrderService;

    const ID: &'static str = SERVICE_CAPABILITY_ID;
    const VERSION: &'static str = VERSION;
}

struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> u128 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
    }
}

struct SystemClockPlugin {
    descriptor: PluginDescriptor,
}

impl SystemClockPlugin {
    fn new() -> Result<Self, ComposeError> {
        let descriptor = PluginDescriptor::new(plugin_id(CLOCK_PLUGIN_ID)?, version()?)
            .sourced_from(source("kernox-example-system-clock")?)
            .provide(CapabilityOffer::new(capability_id(CLOCK_CAPABILITY_ID)?, version()?))?;
        Ok(Self { descriptor })
    }
}

impl Plugin for SystemClockPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        Box::pin(async move {
            ProvisionSet::new()
                .provide::<ClockCapability>(clock)
                .map_err(|error| PluginError::new(error.tag(), error.to_string()))
        })
    }
}

#[derive(Default)]
struct MemoryOrderStore {
    orders: Mutex<BTreeMap<u64, Order>>,
}

impl OrderStore for MemoryOrderStore {
    fn save(&self, order: Order) {
        self.orders
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(order.id, order);
    }

    fn find(&self, id: u64) -> Option<Order> {
        self.orders.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&id).cloned()
    }
}

struct OrderStorePlugin {
    descriptor: PluginDescriptor,
}

impl OrderStorePlugin {
    fn new() -> Result<Self, ComposeError> {
        let descriptor = PluginDescriptor::new(plugin_id(STORE_PLUGIN_ID)?, version()?)
            .sourced_from(source("kernox-example-order-store")?)
            .provide(CapabilityOffer::new(capability_id(STORE_CAPABILITY_ID)?, version()?))?;
        Ok(Self { descriptor })
    }
}

impl Plugin for OrderStorePlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let store: Arc<dyn OrderStore> = Arc::new(MemoryOrderStore::default());
        Box::pin(async move {
            ProvisionSet::new()
                .provide::<OrderStoreCapability>(store)
                .map_err(|error| PluginError::new(error.tag(), error.to_string()))
        })
    }
}

struct DefaultOrderService {
    clock: Arc<dyn Clock>,
    store: Arc<dyn OrderStore>,
    next_id: AtomicU64,
}

impl OrderService for DefaultOrderService {
    fn create(&self, sku: String) -> Order {
        let order = Order {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            sku,
            created_at_millis: self.clock.now_millis(),
        };
        self.store.save(order.clone());
        order
    }
}

struct OrderServicePlugin {
    descriptor: PluginDescriptor,
}

impl OrderServicePlugin {
    fn new() -> Result<Self, ComposeError> {
        let requirement = VersionReq::parse("^1.0")?;
        let descriptor = PluginDescriptor::new(plugin_id(SERVICE_PLUGIN_ID)?, version()?)
            .sourced_from(source("kernox-example-order-service")?)
            .provide(CapabilityOffer::new(capability_id(SERVICE_CAPABILITY_ID)?, version()?))?
            .require(CapabilityRequirement::exactly_one(
                capability_id(CLOCK_CAPABILITY_ID)?,
                requirement.clone(),
            ))?
            .require(CapabilityRequirement::exactly_one(
                capability_id(STORE_CAPABILITY_ID)?,
                requirement,
            ))?;
        Ok(Self { descriptor })
    }
}

impl Plugin for OrderServicePlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let clock = context.require::<ClockCapability>();
        let store = context.require::<OrderStoreCapability>();
        Box::pin(async move {
            let service: Arc<dyn OrderService> = Arc::new(DefaultOrderService {
                clock: clock.map_err(access_failure)?,
                store: store.map_err(access_failure)?,
                next_id: AtomicU64::new(1),
            });
            ProvisionSet::new()
                .provide::<OrderServiceCapability>(service)
                .map_err(|error| PluginError::new(error.tag(), error.to_string()))
        })
    }
}

/// Builds the same three-plugin graph for every Host.
///
/// # Errors
///
/// Returns [`ComposeError`] if built-in contracts or graph resolution fail.
pub fn compose() -> Result<ResolvedApp, ComposeError> {
    Ok(AppBuilder::new()
        .plugin(OrderServicePlugin::new()?)
        .plugin(OrderStorePlugin::new()?)
        .plugin(SystemClockPlugin::new()?)
        .resolve()?)
}

/// Returns the stable provider identity for the order service export.
///
/// # Errors
///
/// Returns an identifier error only if this example's built-in constant is invalid.
pub fn order_service_plugin_id() -> Result<PluginId, kernox::core::IdentifierError> {
    plugin_id(SERVICE_PLUGIN_ID)
}

fn plugin_id(value: &str) -> Result<PluginId, kernox::core::IdentifierError> {
    PluginId::new(value)
}

fn capability_id(value: &str) -> Result<CapabilityId, kernox::core::IdentifierError> {
    CapabilityId::new(value)
}

fn version() -> Result<Version, semver::Error> {
    Version::parse(VERSION)
}

fn source(package: &str) -> Result<PluginSource, kernox::core::DescriptorError> {
    PluginSource::new(package, Some("https://github.com/SylphxAI/kernox".to_owned()))
}

fn access_failure(error: kernox::runtime::AccessError) -> PluginError {
    let tag = error.tag();
    let message = error.to_string();
    drop(error);
    PluginError::new(tag, message)
}
