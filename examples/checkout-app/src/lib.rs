//! Checkout domain composed with two interchangeable payment providers.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use kernox::{
    AppBuilder, Binding, BoxFuture, Capability, CapabilityId, CapabilityOffer,
    CapabilityRequirement, InitializationContext, Plugin, PluginDescriptor, PluginError, PluginId,
    ProvisionSet, ResolvedApp,
};
use semver::{Version, VersionReq};
use thiserror::Error;

use kernox::core::PluginSource;

const CONTRACT_VERSION: &str = "1.0.0";
const CHECKOUT_PLUGIN_ID: &str = "dev.kernox.examples.checkout.service";
const INVENTORY_PLUGIN_ID: &str = "dev.kernox.examples.checkout.inventory";
const CARD_PLUGIN_ID: &str = "dev.kernox.examples.checkout.card-payment";
const WALLET_PLUGIN_ID: &str = "dev.kernox.examples.checkout.wallet-payment";
const CHECKOUT_CAPABILITY_ID: &str = "dev.kernox.examples.checkout.service";
const INVENTORY_CAPABILITY_ID: &str = "dev.kernox.examples.checkout.inventory";
const PAYMENT_CAPABILITY_ID: &str = "dev.kernox.examples.checkout.payment";

/// Payment provider selected at the composition boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaymentProvider {
    /// Select the card adapter.
    Card,
    /// Select the wallet adapter.
    Wallet,
}

impl PaymentProvider {
    /// Returns the provider's stable plugin identity.
    ///
    /// # Errors
    ///
    /// Returns [`kernox::core::IdentifierError`] only if the built-in identity
    /// constant is invalid.
    pub fn plugin_id(self) -> Result<PluginId, kernox::core::IdentifierError> {
        plugin_id(match self {
            Self::Card => CARD_PLUGIN_ID,
            Self::Wallet => WALLET_PLUGIN_ID,
        })
    }

    /// Returns the human-readable provider name used by the receipt.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Card => "card",
            Self::Wallet => "wallet",
        }
    }
}

/// A completed checkout operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    /// Purchased stock-keeping unit.
    pub sku: String,
    /// Amount charged in the example's minor currency unit.
    pub cents: u64,
    /// Provider selected by the graph binding.
    pub provider: &'static str,
}

/// Failure returned by an application-owned payment adapter.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PaymentError {
    /// The provider declined the charge.
    #[error("payment declined by {provider}")]
    Declined {
        /// Stable provider name.
        provider: &'static str,
    },
}

/// Inventory port owned by the checkout domain.
pub trait Inventory: Send + Sync {
    /// Reserves one SKU for the current checkout.
    fn reserve(&self, sku: &str) -> bool;
}

/// Payment port owned by the checkout domain.
pub trait PaymentGateway: Send + Sync {
    /// Returns the provider name used for receipts and diagnostics.
    fn name(&self) -> &'static str;
    /// Charges one amount in the example's minor currency unit.
    ///
    /// # Errors
    ///
    /// Returns [`PaymentError`] when the selected provider declines the charge.
    fn charge(&self, cents: u64) -> Result<(), PaymentError>;
}

/// Checkout use-case port consumed by the host/application shell.
pub trait Checkout: Send + Sync {
    /// Reserves inventory and charges the selected provider.
    ///
    /// # Errors
    ///
    /// Returns [`PurchaseError`] when inventory is unavailable or payment is
    /// declined.
    fn purchase(&self, sku: String, cents: u64) -> Result<Receipt, PurchaseError>;
}

/// Failure returned by the checkout use case.
#[derive(Debug, Error, PartialEq)]
pub enum PurchaseError {
    /// The inventory adapter could not reserve the requested SKU.
    #[error("SKU {sku:?} is unavailable")]
    OutOfStock {
        /// Requested SKU.
        sku: String,
    },
    /// The selected payment adapter rejected the charge.
    #[error(transparent)]
    Payment(#[from] PaymentError),
}

/// Marker for the inventory port.
pub struct InventoryCapability;

impl Capability for InventoryCapability {
    type Interface = dyn Inventory;

    const ID: &'static str = INVENTORY_CAPABILITY_ID;
    const VERSION: &'static str = CONTRACT_VERSION;
}

/// Marker for the payment port.
pub struct PaymentGatewayCapability;

impl Capability for PaymentGatewayCapability {
    type Interface = dyn PaymentGateway;

    const ID: &'static str = PAYMENT_CAPABILITY_ID;
    const VERSION: &'static str = CONTRACT_VERSION;
}

/// Marker for the checkout use-case port.
pub struct CheckoutCapability;

impl Capability for CheckoutCapability {
    type Interface = dyn Checkout;

    const ID: &'static str = CHECKOUT_CAPABILITY_ID;
    const VERSION: &'static str = CONTRACT_VERSION;
}

/// Errors constructing the example's static graph.
#[derive(Debug, Error)]
pub enum ComposeError {
    /// A built-in identifier is invalid.
    #[error(transparent)]
    Identifier(#[from] kernox::core::IdentifierError),
    /// A descriptor declaration is inconsistent.
    #[error(transparent)]
    Descriptor(#[from] kernox::core::DescriptorError),
    /// A built-in semantic version is invalid.
    #[error(transparent)]
    Version(#[from] semver::Error),
    /// The selected graph is invalid.
    #[error(transparent)]
    Resolve(#[from] kernox::core::ResolveError),
}

struct MemoryInventory {
    available: Mutex<BTreeSet<String>>,
}

impl MemoryInventory {
    fn new() -> Self {
        Self {
            available: Mutex::new(BTreeSet::from([
                "kernox-book".to_owned(),
                "kernox-sticker".to_owned(),
            ])),
        }
    }
}

impl Inventory for MemoryInventory {
    fn reserve(&self, sku: &str) -> bool {
        self.available.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(sku)
    }
}

struct CardPayment;

impl PaymentGateway for CardPayment {
    fn name(&self) -> &'static str {
        "card"
    }

    fn charge(&self, _cents: u64) -> Result<(), PaymentError> {
        Ok(())
    }
}

struct WalletPayment;

impl PaymentGateway for WalletPayment {
    fn name(&self) -> &'static str {
        "wallet"
    }

    fn charge(&self, _cents: u64) -> Result<(), PaymentError> {
        Ok(())
    }
}

struct CheckoutService {
    inventory: Arc<dyn Inventory>,
    payment: Arc<dyn PaymentGateway>,
}

impl Checkout for CheckoutService {
    fn purchase(&self, sku: String, cents: u64) -> Result<Receipt, PurchaseError> {
        if !self.inventory.reserve(&sku) {
            return Err(PurchaseError::OutOfStock { sku });
        }
        self.payment.charge(cents)?;
        Ok(Receipt { sku, cents, provider: self.payment.name() })
    }
}

struct InventoryPlugin {
    descriptor: PluginDescriptor,
}

impl InventoryPlugin {
    fn new() -> Result<Self, ComposeError> {
        let descriptor = PluginDescriptor::new(plugin_id(INVENTORY_PLUGIN_ID)?, version()?)
            .sourced_from(source("kernox-example-checkout-inventory")?)
            .provide(CapabilityOffer::new(capability_id(INVENTORY_CAPABILITY_ID)?, version()?))?;
        Ok(Self { descriptor })
    }
}

impl Plugin for InventoryPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let inventory: Arc<dyn Inventory> = Arc::new(MemoryInventory::new());
        Box::pin(async move {
            ProvisionSet::new()
                .provide::<InventoryCapability>(inventory)
                .map_err(|error| PluginError::new(error.tag(), error.to_string()))
        })
    }
}

struct PaymentPlugin {
    descriptor: PluginDescriptor,
    provider: PaymentProvider,
}

impl PaymentPlugin {
    fn card() -> Result<Self, ComposeError> {
        Self::new(PaymentProvider::Card)
    }

    fn wallet() -> Result<Self, ComposeError> {
        Self::new(PaymentProvider::Wallet)
    }

    fn new(provider: PaymentProvider) -> Result<Self, ComposeError> {
        let (plugin, package) = match provider {
            PaymentProvider::Card => (CARD_PLUGIN_ID, "kernox-example-checkout-card"),
            PaymentProvider::Wallet => (WALLET_PLUGIN_ID, "kernox-example-checkout-wallet"),
        };
        let descriptor = PluginDescriptor::new(plugin_id(plugin)?, version()?)
            .sourced_from(source(package)?)
            .provide(CapabilityOffer::new(capability_id(PAYMENT_CAPABILITY_ID)?, version()?))?;
        Ok(Self { descriptor, provider })
    }
}

impl Plugin for PaymentPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let gateway: Arc<dyn PaymentGateway> = match self.provider {
            PaymentProvider::Card => Arc::new(CardPayment),
            PaymentProvider::Wallet => Arc::new(WalletPayment),
        };
        Box::pin(async move {
            ProvisionSet::new()
                .provide::<PaymentGatewayCapability>(gateway)
                .map_err(|error| PluginError::new(error.tag(), error.to_string()))
        })
    }
}

struct CheckoutPlugin {
    descriptor: PluginDescriptor,
}

impl CheckoutPlugin {
    fn new() -> Result<Self, ComposeError> {
        let requirement = VersionReq::parse("^1.0")?;
        let descriptor = PluginDescriptor::new(plugin_id(CHECKOUT_PLUGIN_ID)?, version()?)
            .sourced_from(source("kernox-example-checkout-service")?)
            .provide(CapabilityOffer::new(capability_id(CHECKOUT_CAPABILITY_ID)?, version()?))?
            .require(CapabilityRequirement::exactly_one(
                capability_id(INVENTORY_CAPABILITY_ID)?,
                requirement.clone(),
            ))?
            .require(CapabilityRequirement::exactly_one(
                capability_id(PAYMENT_CAPABILITY_ID)?,
                requirement,
            ))?;
        Ok(Self { descriptor })
    }
}

impl Plugin for CheckoutPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let inventory = context.require::<InventoryCapability>();
        let payment = context.require::<PaymentGatewayCapability>();
        Box::pin(async move {
            let service: Arc<dyn Checkout> = Arc::new(CheckoutService {
                inventory: inventory.map_err(access_failure)?,
                payment: payment.map_err(access_failure)?,
            });
            ProvisionSet::new()
                .provide::<CheckoutCapability>(service)
                .map_err(|error| PluginError::new(error.tag(), error.to_string()))
        })
    }
}

/// Builds the checkout graph with an explicit payment-provider binding.
///
/// # Errors
///
/// Returns [`ComposeError`] when a built-in descriptor, version, binding, or
/// graph contract is invalid.
pub fn compose(provider: PaymentProvider) -> Result<ResolvedApp, ComposeError> {
    let checkout = plugin_id(CHECKOUT_PLUGIN_ID)?;
    let payment = provider.plugin_id()?;
    Ok(AppBuilder::new()
        .plugin(CheckoutPlugin::new()?)
        .plugin(InventoryPlugin::new()?)
        .plugin(PaymentPlugin::card()?)
        .plugin(PaymentPlugin::wallet()?)
        .binding(Binding::new(checkout, capability_id(PAYMENT_CAPABILITY_ID)?, payment))
        .resolve()?)
}

/// Returns the provider identity used for the checkout root export.
///
/// # Errors
///
/// Returns an identifier error only if the built-in identity constant is
/// invalid.
pub fn checkout_plugin_id() -> Result<PluginId, kernox::core::IdentifierError> {
    plugin_id(CHECKOUT_PLUGIN_ID)
}

fn plugin_id(value: &str) -> Result<PluginId, kernox::core::IdentifierError> {
    PluginId::new(value)
}

fn capability_id(value: &str) -> Result<CapabilityId, kernox::core::IdentifierError> {
    CapabilityId::new(value)
}

fn version() -> Result<Version, semver::Error> {
    Version::parse(CONTRACT_VERSION)
}

fn source(package: &str) -> Result<PluginSource, kernox::core::DescriptorError> {
    PluginSource::new(
        package,
        Some("https://github.com/SylphxAI/kernox/tree/main/examples/checkout-app".to_owned()),
    )
}

fn access_failure(error: kernox::runtime::AccessError) -> PluginError {
    let tag = error.tag();
    let message = error.to_string();
    drop(error);
    PluginError::new(tag, message)
}
