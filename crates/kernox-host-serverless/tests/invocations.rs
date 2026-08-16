//! Warm application and fresh invocation isolation tests.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    collections::BTreeSet,
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use futures::executor::block_on;
use kernox_host_serverless::{
    InvocationAdmissionError, InvocationError, ServerlessConfig, ServerlessConfigError,
    ServerlessHost,
};
use kernox_runtime::AppBuilder;

#[test]
fn rejects_zero_concurrency_capacity() {
    block_on(async {
        let app = AppBuilder::new().resolve().unwrap().start().await.unwrap();
        let error = ServerlessHost::new(app, ServerlessConfig { max_concurrent_invocations: 0 })
            .err()
            .expect("configuration must fail");
        assert_eq!(error, ServerlessConfigError);
    });
}

#[test]
fn every_warm_call_gets_fresh_request_state_under_concurrency() {
    block_on(async {
        let app = AppBuilder::new().resolve().unwrap().start().await.unwrap();
        let mut host =
            ServerlessHost::new(app, ServerlessConfig { max_concurrent_invocations: 128 }).unwrap();
        let app_scope = host.app_scope_id();
        let observed = Arc::new(Mutex::new(Vec::new()));

        let calls = (0_u64..64).map(|request_value| {
            let observed = Arc::clone(&observed);
            host.invoke(None, move |context| {
                Box::pin(async move {
                    assert_eq!(context.scope().parent(), Some(app_scope));
                    observed.lock().unwrap().push((context.scope().id(), request_value));
                    Ok::<_, Infallible>(request_value)
                })
            })
        });
        let results = futures::future::join_all(calls).await;

        assert!(results.iter().all(Result::is_ok));
        assert_eq!(host.active_invocations(), 0);
        {
            let observed = observed.lock().unwrap();
            let ids: BTreeSet<_> = observed.iter().map(|(id, _)| *id).collect();
            let values: BTreeSet<_> = observed.iter().map(|(_, value)| *value).collect();
            assert_eq!(ids.len(), 64);
            assert_eq!(values.len(), 64);
        }
        assert!(host.shutdown().await.is_clean());
        assert_eq!(
            host.begin_invocation(None).err().expect("admission must be closed"),
            InvocationAdmissionError::Closed
        );
    });
}

#[test]
fn capacity_and_deadline_are_explicit_per_invocation() {
    block_on(async {
        let app = AppBuilder::new().resolve().unwrap().start().await.unwrap();
        let host =
            ServerlessHost::new(app, ServerlessConfig { max_concurrent_invocations: 1 }).unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        let held = host.begin_invocation(Some(deadline)).unwrap();

        assert_eq!(held.context().deadline(), Some(deadline));
        assert!(!held.context().deadline_exceeded());
        assert_eq!(host.active_invocations(), 1);
        assert_eq!(
            host.begin_invocation(None).err().expect("capacity must fail"),
            InvocationAdmissionError::Capacity
        );
        drop(held);
        assert_eq!(host.active_invocations(), 0);
        assert!(host.begin_invocation(None).is_ok());
    });
}

#[test]
fn expired_deadline_rejects_before_handler_admission() {
    block_on(async {
        let app = AppBuilder::new().resolve().unwrap().start().await.unwrap();
        let host =
            ServerlessHost::new(app, ServerlessConfig { max_concurrent_invocations: 1 }).unwrap();
        let called = Arc::new(AtomicBool::new(false));
        let expired = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .expect("a one-second subtraction must remain representable");
        let result = host
            .invoke(Some(expired), {
                let called = Arc::clone(&called);
                move |_| {
                    Box::pin(async move {
                        called.store(true, Ordering::Release);
                        Ok::<_, Infallible>(())
                    })
                }
            })
            .await;

        assert!(matches!(
            result,
            Err(InvocationError::Admission(InvocationAdmissionError::DeadlineExceeded))
        ));
        assert_eq!(
            InvocationAdmissionError::DeadlineExceeded.tag(),
            "serverless.deadline-exceeded"
        );
        assert!(!called.load(Ordering::Acquire));
        assert_eq!(host.active_invocations(), 0);
    });
}
