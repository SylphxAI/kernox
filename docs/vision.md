# Kernox Vision

**Status:** Canonical product destination
**Identity graph:** [`capabilities.md`](capabilities.md)
**ADR:** [`adr/20260815T185400Z-static-capability-graph.md`](adr/20260815T185400Z-static-capability-graph.md)

This document owns the long-term product destination. It does not claim the destination is landed or live.

## Finished product

Kernox is an experimental embeddable graph-backed application kernel for Rust.
A product is a statically selected set of plugins. Each plugin declares
versioned capabilities it offers and requires; Kernox validates the graph,
injects typed handles, and owns deterministic startup, rollback, and shutdown.

The graph is the control plane, not the request path. After boot, domain code
calls an ordinary `Arc<dyn Trait>` directly — no graph traversal,
serialization, event bus, or service-locator lookup per call. The absolute
point-estimate delta against direct composition is 0.14% on the recorded
baseline, with overlapping confidence intervals.

Hosts own the outer execution model: long-lived Tokio (`kernox-host-tokio`),
warm serverless (`kernox-host-serverless`), and a deterministic test host
(`kernox-testkit`). CLI products compose the same graph through `AppBuilder`
without Tokio; they are not a fourth host crate. `cargo-kernox` is inspection
tooling, not a host.

Kernox is not a default company dependency. Other products must not silently
take it. Production coupling of any other product needs an accepted decision.
It is not a Keel replacement, not a central plugin marketplace, not
WASM-first, and not Sylphx-only.

The public package train is pre-1.0 `0.1.x`. Stable 1.x is refused until the
engine is admitted mature.

## Users

- **Rust product engineers** composing modular monoliths, services, workers,
  CLIs, serverless functions, or game/application hosts — inside or outside
  Sylphx.
- **Library authors** publishing reusable domain, adapter, or host plugins.
- **Platform engineers** exposing existing services through replaceable
  adapters without moving service authority into Kernox.

## Product oracle

The destination is true only when all of the following hold at their named
layer.

**Source.** A three-plugin application composes via descriptors, validates the
capability DAG deterministically (stable startup/teardown order independent of
insertion), publishes typed handles atomically, rolls back in reverse order on
failure, and runs under Tokio, warm serverless, CLI-without-Tokio, and
deterministic test hosts. Tokio tasks are supervised with cancellation and
drain. Locked builds, benchmarks, and provenance hold in the current source
contract. `cargo kernox check --verified` on the reference fixtures must
hold. A crate publish or `cargo test` green alone is not this oracle.

**Released.** A tagged `0.1.x` workspace version that is an ancestor of `main`
is published by the tag-gated trusted-publishing writer and read back from
crates.io with checksum equal to that tag's `release-manifest.json`.
Crate-name existence at `0.0.1` is not this oracle.

**Live.** Independently released applications compose at least three separately
owned plugins, pass Kernox conformance, and do not fork or patch Kernox core.
That live count is the North Star Metric below, not a graph identity.

## North Star Metric

**Verified applications** counts independently released applications that
compose at least three separately owned plugins, pass Kernox conformance, and
do not fork or patch Kernox core.

The in-tree three-plugin fixtures, examples, and `cargo kernox check
--verified` are a local measurement of the composition and conformance
predicates. They are not a live count of independently owned released
applications.

GitHub stars, crate downloads, plugin count without reuse, successful
compilation without lifecycle proof, and green CI without a usable application
path may help diagnose the product but cannot replace the North Star.

This metric is not an identity in [`capabilities.md`](capabilities.md). It
cannot close as a graph `Done when` while other products are forbidden from
silently taking Kernox and production coupling still needs an accepted
decision. Independent verified applications remain a live observation after
that coupling decision.

## Non-goals

- HTTP, storage, identity, AI, billing, ORM, a generic event bus, or business
  policy in the kernel.
- A second product tree or a runtime service-locator per call.
- A fourth CLI host crate (`kernox-host-cli`). CLI products compose through
  `AppBuilder` without Tokio.
- Out-of-process or WebAssembly extension that weakens the native static path
  before an accepted ABI/WIT, capability-grant, resource, and migration
  contract.
- Native dynamic-library loading, or claims that trusted native plugins are
  isolated.
- Replacing Keel, or becoming a central plugin marketplace.
- Becoming a default company engine other products must depend on. Production
  coupling of any other product needs an accepted decision.
- Sylphx-only composition. Any Rust product may embed Kernox; Sylphx products
  do not get a privileged path.
- Stable 1.x publication before the engine is admitted mature.
