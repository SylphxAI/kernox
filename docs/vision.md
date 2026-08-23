# Kernox Vision

**Status:** Canonical product destination
**Identity graph:** [`capabilities.md`](capabilities.md)
**ADR:** [`adr/20260815T185400Z-static-capability-graph.md`](adr/20260815T185400Z-static-capability-graph.md)

This document owns the long-term product destination. It does not claim the destination is landed or live.

## Destination

Kernox is a graph-backed application kernel for Rust. A product is a statically selected set of plugins. Each plugin declares versioned capabilities it offers and requires; Kernox validates the graph, injects typed handles, and owns deterministic startup, rollback, and shutdown.

The graph is the control plane, not the request path. After boot, domain code calls an ordinary `Arc<dyn Trait>` directly — no graph traversal, serialization, event bus, or service-locator lookup per call. The absolute point-estimate delta against direct composition is 0.14% on the recorded baseline, with overlapping confidence intervals. Hosts (Tokio long-lived, warm serverless, CLI, deterministic test host) own the outer execution model.

## Users and their jobs

- **Rust product engineers** composing modular monoliths, services, workers, CLIs, serverless functions, or game/application hosts.
- **Library authors** publishing reusable domain, adapter, or host plugins.
- **Platform engineers** exposing existing services through replaceable adapters without moving service authority into Kernox.

## Not doing

- HTTP, storage, identity, AI, billing, ORM, generic event bus, or business policy.
- Out-of-process or WebAssembly extension that weakens the native static path before implementation.
- A second product tree or a runtime service-locator per call.

## Product oracle

The destination is true only when a three-plugin application can compose via descriptors, validate the capability DAG deterministically (stable startup/teardown order independent of insertion), publish typed handles atomically, roll back in reverse order on failure, supervise Tokio tasks with cancellation/drain, and prove locked builds, benchmarks, and provenance in the current source contract.

A crate publish or `cargo test` green alone is not the whole oracle; `cargo kernox check --verified` on the reference fixtures must hold.
