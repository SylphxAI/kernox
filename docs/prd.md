# Kernox product requirements

## Product promise

A product author selects a Host and a set of Plugins. Kernox constructs one
deterministic capability graph, rejects invalid compositions before work is
served, initializes and starts plugins in dependency order, and tears them down
in reverse order. After injection, normal application calls use direct typed
handles.

## Users

- Rust product engineers composing modular monoliths, services, workers, CLIs,
  serverless functions, or game/application hosts.
- Library authors publishing reusable domain, adapter, or host plugins.
- Platform engineers exposing existing services through replaceable adapters
  without moving service authority into Kernox.

## Invariants

1. The graph is control-plane state; ordinary application calls never traverse
   it.
2. Plugin identity and capability contracts are stable, validated, and
   deterministic.
3. A required capability has exactly one selected compatible provider;
   absence, ambiguity, conflict, and cycles fail before readiness.
4. A plugin receives only declared dependencies during initialization and keeps
   direct handles thereafter; no ambient global resolver enters domain code.
5. Initialization is transactional. Failure rolls back initialized plugins in
   reverse order and does not publish partial provisions.
6. Lifecycle transition order is deterministic, invalid transitions fail, and
   shutdown is idempotent.
7. Plugins close their own admission and resources through lifecycle hooks;
   official Host supervisors bound app-wide tasks and report work that does not
   drain. Statically removing a plugin removes its registrations entirely.
8. Native static plugins are trusted in-process code, not a security sandbox.
9. Hosts own the outer execution model. The kernel does not assume HTTP,
   Tokio, Lambda, a frame loop, a filesystem, or process shutdown.
10. Domain and application policy do not depend on Kernox.

## Required capabilities

### KR-001 — Stable identities and descriptors

Validated plugin and capability identifiers, semantic versions and version
requirements, declared provides/requires/conflicts, source attribution, and
bounded metadata. Invalid or duplicate identity fails with typed diagnostics.

### KR-002 — Deterministic graph resolution

Resolve providers, explicit bindings, optional and multi-provider requirements,
conflicts, and dependency order. Produce stable startup/teardown order and an
explainable graph report independent of insertion order.

### KR-003 — Typed provisioning and injection

Plugins publish typed capability handles atomically during initialization.
Consumers resolve only declared capabilities. Type/descriptor disagreement is
rejected, and the resolver cannot escape the initialization borrow.

### KR-004 — Transactional lifecycle

Support compose, validate, initialize, start, ready, quiesce, stop, and dispose
semantics with typed state and failure reports. Partial initialization/startup
must unwind safely in reverse dependency order.

### KR-005 — Scoped ownership and cancellation

Provide host-neutral scope identity and closure semantics plus an official
Tokio task scope with cancellation, draining, timeout, and leak reporting.

### KR-006 — Host SDK

Ship long-lived Tokio, provider-neutral serverless invocation, CLI, and
deterministic test hosts. Host-specific dependencies must not enter the core.

### KR-007 — Diagnostics and observability

Every graph and lifecycle failure has a stable error tag, structured context,
and human-readable explanation. Expose privacy-safe lifecycle observations
through a small sink contract without importing a telemetry SDK into core.

### KR-008 — Inspection and conformance tooling

Export a versioned graph description and provide CLI validation/rendering,
architecture checks, reference fixtures, and a plugin conformance testkit.

### KR-009 — Compatibility and extension ladder

Define source compatibility, descriptor-schema compatibility, deprecation, and
host capability negotiation. Native static composition is first. Out-of-process
and WebAssembly Component extension designs must preserve the same semantic
contract and must not weaken the native path before implementation.

### KR-010 — Commercial release discipline

Provide reproducible locked builds, public API documentation, dual licensing,
security policy, dependency/advisory/license checks, semantic-version checking,
benchmarks, fuzz/property testing, changelog, provenance-ready release
automation, and examples exercising real application paths.

## Explicit non-goals

- Business capabilities such as HTTP, identity, storage, billing, AI, queues,
  or workflow policy in the kernel.
- A generic event bus, service locator, ORM, broker, service mesh, or deployment
  control plane.
- Arbitrary native dynamic-library loading or claims that trusted native
  plugins are isolated.
- Forcing every function, entity, adapter, or crate to become a plugin.
- Making different domain semantics reusable merely by packaging them alike.

## Release terminal

The first public production release is admitted only when every required
capability above has executable evidence in the acceptance matrix, the public
API is reviewed for 1.x compatibility, all reference applications pass, and
the release artifact is published and read back from its registry. There is no
reduced MVP release path.
