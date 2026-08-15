# Kernox runtime contract

## Vocabulary

- **Plugin descriptor:** stable identity, version, provided/required
  capabilities, conflicts, and bounded metadata.
- **Resolved graph:** immutable selected provider graph plus deterministic
  startup and teardown order.
- **Provision:** a typed capability handle published by one initialized plugin.
- **Binding:** an explicit product choice selecting a provider for one consumer
  requirement.
- **Scope:** an ownership boundary with identity, cancellation/closure state,
  and host-managed resources.
- **Host:** the outer invocation/loop/runtime integration.

## Identifier contract

Identifiers are lowercase reverse-domain-like dotted names. Each segment starts
with an ASCII lowercase letter and continues with lowercase letters, digits,
or hyphens. Total and segment lengths are bounded. Display names and labels are
metadata and never identity.

## Resolution contract

Inputs are validated before graph construction. Resolution is pure and has no
I/O. Stable lexical identity breaks otherwise independent topological ties.
Every failure carries a stable tag, subject identities, and enough structured
context to repair the composition without parsing display text.

Resolution outputs:

- selected provider for each single/optional requirement;
- ordered providers for each multi requirement;
- dependency edges with requirement attribution;
- startup and reverse teardown order;
- unused provider and optional-miss diagnostics; and
- a schema-versioned inspectable graph report.

## Provision contract

The supported typed API keys provisions by Rust `TypeId` and the declared
capability identity. A plugin may resolve only requirements declared by its own
descriptor. Initialization uses a non-owning resolver view that cannot be
stored as `'static`. Staged provisions are committed only after all declared
offers are present exactly once and their identities/versions agree.

The registry is immutable after readiness. Runtime reconfiguration is not a
native V1 operation.

## Lifecycle state machine

```text
declared -> resolved -> initializing -> initialized -> starting -> ready
ready -> quiescing -> stopping -> disposed
initializing|starting -> rolling-back -> disposed-with-failure
```

Invalid transitions return typed errors. Expected hook failures never panic.
Panics in trusted native plugin code are process failures unless the Host has a
separate isolation boundary; Kernox does not promise unwind-safe containment.

Rollback records the primary failure and every cleanup failure without losing
either. Cleanup continues after an individual cleanup error. A repeated
shutdown returns the prior terminal report and performs no second effect.

## Scope contract

Every scope has a stable opaque identity and parent. Closing a parent prevents
new child/resource registration, propagates cancellation, waits within the
host budget, and reports resources that did not drain. Scope-local data must
not escape its lifetime through supported APIs.

The core represents scope state without requiring Tokio. Runtime-specific task
supervision belongs to host packages.

## Host contract

- A long-lived Host starts one App Scope, accepts work only after readiness,
  stops new work during quiesce, drains, then shuts down.
- A serverless Host may reuse one App Scope across warm invocations and creates
  a fresh Invocation Scope for every call. Correctness never depends on a
  shutdown callback.
- A deterministic Test Host controls clock, order, cancellation, and injected
  failures without network or process signals.
- Host capability negotiation fails before readiness when a Plugin requires an
  unsupported runtime property.

## Observability contract

Core emits typed lifecycle observations to an optional sink. Events contain
stable operation/plugin/scope identities, transition, outcome, duration, and
error tag; they contain no secrets, arbitrary plugin payloads, or telemetry SDK
types. Host adapters may translate them to OpenTelemetry.

## Compatibility

- Rust public APIs follow semantic versioning.
- Descriptor report/schema changes are versioned independently and old readers
  reject unsupported major versions.
- Error tags are stable within a major version; display prose is not a machine
  contract.
- Deprecations name a replacement and remain for at least one minor release
  before the next permitted major removal.
- Runtime-isolated plugins will require an explicit ABI compatibility contract;
  native crate compatibility does not imply it.
