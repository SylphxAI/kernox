use std::{future::Future, pin::Pin};

use kernox_core::PluginDescriptor;

use crate::{HostRequirement, InitializationContext, PluginError, ProvisionSet, ScopeView};

/// Boxed future used by the object-safe plugin lifecycle contract.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Runtime-neutral context supplied to non-initialization lifecycle hooks.
#[derive(Clone, Copy, Debug)]
pub struct LifecycleContext<'a> {
    scope: ScopeView<'a>,
}

impl<'a> LifecycleContext<'a> {
    pub(crate) const fn new(scope: ScopeView<'a>) -> Self {
        Self { scope }
    }

    /// Returns the non-owning application scope view.
    #[must_use]
    pub const fn scope(self) -> ScopeView<'a> {
        self.scope
    }
}

/// Statically linked unit of composition with explicit lifecycle ownership.
pub trait Plugin: Send + Sync + 'static {
    /// Returns the immutable composition contract for this instance.
    fn descriptor(&self) -> &PluginDescriptor;

    /// Returns static requirements on the selected Host runtime.
    ///
    /// Kernox evaluates this once beside [`Plugin::descriptor`] during
    /// composition. Host requirements are negotiation metadata, not graph
    /// provisions and are never consulted by normal application calls.
    fn host_requirements(&self) -> Vec<HostRequirement> {
        Vec::new()
    }

    /// Builds declared provisions using only declared, already-ready dependencies.
    ///
    /// An unwind from this method or its future becomes `plugin.hook-panicked`
    /// and rolls back already initialized plugins. It is not plugin isolation.
    fn initialize<'a>(
        &'a mut self,
        context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>>;

    /// Activates work after every provision transaction has committed.
    fn start<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async { Ok(()) })
    }

    /// Stops accepting new work before shutdown.
    fn quiesce<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async { Ok(()) })
    }

    /// Stops plugin-owned work.
    fn stop<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async { Ok(()) })
    }

    /// Releases plugin-owned resources. This must tolerate partial initialization.
    fn dispose<'a>(
        &'a mut self,
        _context: LifecycleContext<'a>,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async { Ok(()) })
    }
}
