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
- unselected-offer and optional-miss diagnostics; and
- a schema-versioned inspectable graph report whose readers reject an
  unsupported report major independently of the composition-input schema.

## Provision contract

The supported typed API keys provisions by Rust `TypeId` and the declared
capability identity. A plugin may resolve only requirements declared by its own
descriptor. Initialization uses a non-owning resolver view that cannot be
stored as `'static`. Staged provisions are committed only after all declared
offers are present exactly once and their identities/versions agree.

Each descriptor is snapshotted exactly once before resolution and becomes the
canonical contract used for provisioning. The registry is immutable after
readiness. Runtime reconfiguration is not a native V1 operation.

## Lifecycle state machine

```text
declared -> resolved -> initializing -> initialized -> starting -> ready
ready -> quiescing -> stopping -> disposed
initializing|starting -> rolling-back -> disposed-with-failure
```

The consuming `ResolvedApp -> RunningApp` API makes invalid startup transitions
unrepresentable; shutdown caches one terminal report. Expected hook failures
return typed errors. If a plugin hook function or its future unwinds, the
lifecycle executor converts that unwind into a typed `plugin.hook-panicked`
failure without retaining the panic payload, then follows the same rollback or
continued-cleanup path as an expected error. Observation sink unwinds are
discarded so they cannot abort remaining hooks. This is lifecycle-executor
supervision, not plugin isolation or process-wide unwind containment.

Root capability acquisition closes at the same admission boundary: once the
application scope enters `Closing`, `RunningApp::capability_from` fails with
`access.application-unavailable`, even while cleanup hooks are still running.
Handles acquired before that boundary remain direct and are not revocable
without an explicit indirection contract.

Rollback records the primary failure and every cleanup failure without losing
either. Cleanup continues after an individual cleanup error or hook unwind. A
repeated shutdown returns the prior terminal report and performs no second
effect.

## Scope contract

Every scope has a stable opaque identity and parent. Closing a parent prevents
new child registration and propagates closure state. Scope-local views cannot
escape their invocation lifetime through supported APIs.

The core represents scope state without requiring Tokio. Cancellation,
resource registration, bounded draining, and leak reporting belong to the Host
package that owns those resources.

## Host contract

- A long-lived Host starts one App Scope, accepts work only after readiness,
  stops new work during quiesce, drains, then shuts down.
- A serverless Host may reuse one App Scope across warm invocations and creates
  a fresh Invocation Scope for every call. Correctness never depends on a
  shutdown callback.
- A Tokio Host reports tasks that exceed the cooperative drain budget only
  after forced abort has destroyed their tracked futures.
- The deterministic testkit records lifecycle order and injects typed failures
  without network or process signals; domain clocks remain ordinary test
  capabilities rather than kernel policy.
- Host capability negotiation fails before readiness when a Plugin requires an
  unsupported runtime property.

## Observability contract

Runtime emits typed lifecycle observations to an optional sink. Events contain
stable plugin/scope identities, phase, outcome, duration, and error tag; they
contain no secrets, arbitrary plugin payloads, or telemetry SDK types. Sink
implementations must be fast and must not panic. Host adapters may translate
them to OpenTelemetry.

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
