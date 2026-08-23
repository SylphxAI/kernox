//! Provider-neutral serverless host with warm app reuse and fresh invocation scopes.

use std::{
    fmt,
    marker::PhantomData,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};

use kernox_core::PluginId;
use kernox_runtime::{
    AccessError, BoxFuture, Capability, InvocationScope, RunningApp, ScopeId, ScopeView,
    ShutdownReport,
};
use thiserror::Error;

/// Serverless concurrency policy independent of any cloud provider SDK.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerlessConfig {
    /// Maximum live invocation scopes per warm application instance.
    pub max_concurrent_invocations: usize,
}

impl Default for ServerlessConfig {
    fn default() -> Self {
        Self { max_concurrent_invocations: 1_024 }
    }
}

/// Invalid serverless host configuration.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("max_concurrent_invocations must be greater than zero")]
pub struct ServerlessConfigError;

/// Failure to admit a new invocation scope.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum InvocationAdmissionError {
    /// Host shutdown has closed admission.
    #[error("serverless host is shutting down")]
    Closed,
    /// Configured concurrency capacity is exhausted.
    #[error("serverless invocation capacity is exhausted")]
    Capacity,
    /// The platform deadline had already elapsed before admission.
    #[error("serverless invocation deadline has elapsed")]
    DeadlineExceeded,
}

impl InvocationAdmissionError {
    /// Returns the stable machine-readable diagnostic tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Closed => "serverless.closed",
            Self::Capacity => "serverless.capacity",
            Self::DeadlineExceeded => "serverless.deadline-exceeded",
        }
    }
}

/// Admission or user-handler failure from [`ServerlessHost::invoke`].
#[derive(Debug)]
pub enum InvocationError<E> {
    /// Kernox rejected this invocation before calling user code.
    Admission(InvocationAdmissionError),
    /// The admitted user handler returned an error.
    Handler(E),
}

impl<E: fmt::Display> fmt::Display for InvocationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => error.fmt(formatter),
            Self::Handler(error) => error.fmt(formatter),
        }
    }
}

impl<E> std::error::Error for InvocationError<E> where E: std::error::Error + 'static {}

/// Provider-neutral warm application host.
///
/// Application provisions may be reused across calls. Every admitted call gets
/// a unique child scope and no supported API stores request payloads globally.
pub struct ServerlessHost {
    app: RunningApp,
    config: ServerlessConfig,
    accepting: AtomicBool,
    active: AtomicUsize,
}

impl ServerlessHost {
    /// Wraps one already-ready application for warm invocation reuse.
    ///
    /// # Errors
    ///
    /// Returns [`ServerlessConfigError`] for zero concurrency capacity.
    pub fn new(app: RunningApp, config: ServerlessConfig) -> Result<Self, ServerlessConfigError> {
        if config.max_concurrent_invocations == 0 {
            return Err(ServerlessConfigError);
        }
        Ok(Self { app, config, accepting: AtomicBool::new(true), active: AtomicUsize::new(0) })
    }

    /// Returns the warm application scope identity.
    #[must_use]
    pub fn app_scope_id(&self) -> ScopeId {
        self.app.scope().id()
    }

    /// Returns the current live invocation count.
    #[must_use]
    pub fn active_invocations(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }

    /// Acquires an application provision once at the host composition boundary.
    ///
    /// Keep the returned handle in platform integration code; calls on it are
    /// direct and do not traverse the Kernox graph.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError`] for an invalid provider or marker contract.
    pub fn capability_from<C: Capability>(
        &self,
        provider: &PluginId,
    ) -> Result<std::sync::Arc<C::Interface>, AccessError> {
        self.app.capability_from::<C>(provider)
    }

    /// Begins a fresh invocation with an optional platform deadline.
    ///
    /// # Errors
    ///
    /// Returns [`InvocationAdmissionError`] when shutdown has begun, the
    /// deadline has elapsed, or the configured concurrency bound is reached.
    pub fn begin_invocation(
        &self,
        deadline: Option<Instant>,
    ) -> Result<Invocation<'_>, InvocationAdmissionError> {
        if !self.accepting.load(Ordering::Acquire) {
            return Err(InvocationAdmissionError::Closed);
        }

        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(InvocationAdmissionError::DeadlineExceeded);
        }

        self.active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < self.config.max_concurrent_invocations).then_some(active + 1)
            })
            .map_err(|_| InvocationAdmissionError::Capacity)?;

        if !self.accepting.load(Ordering::Acquire) {
            self.active.fetch_sub(1, Ordering::AcqRel);
            return Err(InvocationAdmissionError::Closed);
        }

        if let Ok(scope) = self.app.invocation_scope() {
            Ok(Invocation {
                scope,
                deadline,
                _counter: InvocationCounter { active: &self.active },
                _host: PhantomData,
            })
        } else {
            self.active.fetch_sub(1, Ordering::AcqRel);
            Err(InvocationAdmissionError::Closed)
        }
    }

    /// Runs one handler inside a fresh invocation scope.
    ///
    /// The handler future may borrow the context but cannot return a supported
    /// scope view that outlives the invocation guard.
    ///
    /// # Errors
    ///
    /// Returns [`InvocationError::Admission`] before user code on capacity,
    /// deadline, or shutdown failure, and [`InvocationError::Handler`] for
    /// user failure.
    pub async fn invoke<T, E, F>(
        &self,
        deadline: Option<Instant>,
        handler: F,
    ) -> Result<T, InvocationError<E>>
    where
        F: for<'invocation> FnOnce(
            InvocationContext<'invocation>,
        ) -> BoxFuture<'invocation, Result<T, E>>,
    {
        let invocation = self.begin_invocation(deadline).map_err(InvocationError::Admission)?;
        let result = handler(invocation.context()).await;
        drop(invocation);
        result.map_err(InvocationError::Handler)
    }

    /// Closes admission and executes normal application shutdown when available.
    ///
    /// Correct serverless plugin behavior must not rely on this method being
    /// called because many platforms terminate warm instances without notice.
    pub async fn shutdown(&mut self) -> ShutdownReport {
        self.accepting.store(false, Ordering::Release);
        self.app.shutdown().await
    }
}

/// Owned admission lease for one fresh invocation scope.
#[must_use = "dropping the invocation closes its scope and releases concurrency capacity"]
pub struct Invocation<'host> {
    scope: InvocationScope<'host>,
    deadline: Option<Instant>,
    _counter: InvocationCounter<'host>,
    _host: PhantomData<&'host ServerlessHost>,
}

impl Invocation<'_> {
    /// Creates a non-owning handler context.
    #[must_use]
    pub fn context(&self) -> InvocationContext<'_> {
        InvocationContext { scope: self.scope.view(), deadline: self.deadline }
    }
}

struct InvocationCounter<'host> {
    active: &'host AtomicUsize,
}

impl Drop for InvocationCounter<'_> {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Non-owning request-local context passed to one serverless handler.
#[derive(Clone, Copy, Debug)]
pub struct InvocationContext<'invocation> {
    scope: ScopeView<'invocation>,
    deadline: Option<Instant>,
}

impl<'invocation> InvocationContext<'invocation> {
    /// Returns this invocation's unique scope view.
    #[must_use]
    pub const fn scope(self) -> ScopeView<'invocation> {
        self.scope
    }

    /// Returns the provider-reported deadline when one exists.
    #[must_use]
    pub const fn deadline(self) -> Option<Instant> {
        self.deadline
    }

    /// Returns whether the supplied deadline has elapsed.
    #[must_use]
    pub fn deadline_exceeded(self) -> bool {
        self.deadline.is_some_and(|deadline| Instant::now() >= deadline)
    }
}
