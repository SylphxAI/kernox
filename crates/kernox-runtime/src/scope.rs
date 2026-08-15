use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicU8, AtomicU64, Ordering},
};

use thiserror::Error;

static NEXT_SCOPE_NAMESPACE: AtomicU64 = AtomicU64::new(1);

/// Opaque process-local scope identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScopeId {
    namespace: u64,
    sequence: u64,
}

/// Semantic scope class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ScopeKind {
    /// One initialized application instance.
    Application,
    /// One serverless or request invocation.
    Invocation,
    /// Host-defined nested unit of work.
    Work,
}

/// Observable scope closure state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ScopeState {
    /// New children and host-managed resources may be registered.
    Open = 0,
    /// Closure has begun and admission is closed.
    Closing = 1,
    /// Host-managed draining has completed.
    Closed = 2,
}

/// Failure to create work below a closing scope.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("scope is closing")]
pub struct ScopeError;

#[derive(Debug)]
struct ScopeInner {
    id: ScopeId,
    kind: ScopeKind,
    parent: Option<Weak<ScopeInner>>,
    state: AtomicU8,
    next_id: Arc<AtomicU64>,
    children: Mutex<Vec<Weak<ScopeInner>>>,
}

#[derive(Clone, Debug)]
pub(crate) struct Scope {
    inner: Arc<ScopeInner>,
}

impl Scope {
    pub(crate) fn application() -> Self {
        let namespace = NEXT_SCOPE_NAMESPACE.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: Arc::new(ScopeInner {
                id: ScopeId { namespace, sequence: 0 },
                kind: ScopeKind::Application,
                parent: None,
                state: AtomicU8::new(ScopeState::Open as u8),
                next_id: Arc::new(AtomicU64::new(1)),
                children: Mutex::new(Vec::new()),
            }),
        }
    }

    pub(crate) fn child(&self, kind: ScopeKind) -> Result<Self, ScopeError> {
        if self.state() != ScopeState::Open {
            return Err(ScopeError);
        }
        let sequence = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let child = Self {
            inner: Arc::new(ScopeInner {
                id: ScopeId { namespace: self.inner.id.namespace, sequence },
                kind,
                parent: Some(Arc::downgrade(&self.inner)),
                state: AtomicU8::new(ScopeState::Open as u8),
                next_id: Arc::clone(&self.inner.next_id),
                children: Mutex::new(Vec::new()),
            }),
        };

        let mut children =
            self.inner.children.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.state() != ScopeState::Open {
            child.begin_close();
            child.finish_close();
            return Err(ScopeError);
        }
        children.push(Arc::downgrade(&child.inner));
        Ok(child)
    }

    pub(crate) fn view(&self) -> ScopeView<'_> {
        ScopeView { inner: &self.inner }
    }

    pub(crate) fn state(&self) -> ScopeState {
        decode_state(self.inner.state.load(Ordering::Acquire))
    }

    pub(crate) fn begin_close(&self) {
        let prior = self.inner.state.swap(ScopeState::Closing as u8, Ordering::AcqRel);
        if prior == ScopeState::Closed as u8 {
            self.inner.state.store(ScopeState::Closed as u8, Ordering::Release);
            return;
        }
        let children =
            self.inner.children.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        for child in children.into_iter().filter_map(|child| child.upgrade()) {
            Self { inner: child }.begin_close();
        }
    }

    pub(crate) fn finish_close(&self) {
        let children =
            self.inner.children.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        for child in children.into_iter().filter_map(|child| child.upgrade()) {
            Self { inner: child }.finish_close();
        }
        self.inner.state.store(ScopeState::Closed as u8, Ordering::Release);
    }
}

/// Non-owning scope view passed to plugin hooks.
#[derive(Clone, Copy, Debug)]
pub struct ScopeView<'a> {
    inner: &'a ScopeInner,
}

impl ScopeView<'_> {
    /// Returns the opaque identity.
    #[must_use]
    pub const fn id(self) -> ScopeId {
        self.inner.id
    }

    /// Returns the semantic scope kind.
    #[must_use]
    pub const fn kind(self) -> ScopeKind {
        self.inner.kind
    }

    /// Returns the parent identity, when present and still alive.
    #[must_use]
    pub fn parent(self) -> Option<ScopeId> {
        self.inner.parent.as_ref().and_then(Weak::upgrade).map(|parent| parent.id)
    }

    /// Returns the current closure state.
    #[must_use]
    pub fn state(self) -> ScopeState {
        decode_state(self.inner.state.load(Ordering::Acquire))
    }

    /// Returns whether new work should be rejected.
    #[must_use]
    pub fn is_closing(self) -> bool {
        self.state() != ScopeState::Open
    }
}

const fn decode_state(value: u8) -> ScopeState {
    match value {
        0 => ScopeState::Open,
        1 => ScopeState::Closing,
        _ => ScopeState::Closed,
    }
}
