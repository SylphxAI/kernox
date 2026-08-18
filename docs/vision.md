# Kernox product vision

This document is the canonical destination for Kernox. It owns the product's
users, boundaries, success condition, and maturity; the capability DAG defines
the architecture required to reach it, and the PRD supplies subordinate detail.

## Destination

Kernox is an experimental, embeddable Rust engine for composing a Host and a
set of trusted in-process Plugins into one deterministic capability graph. It
exists to make capability selection, provisioning, and lifecycle ownership
explicit before an application serves work while leaving product, domain, and
service authority with the owning application.

At the destination, a product author can select plugins, bind intentional
provider choices, and receive a typed failure for an invalid composition before
any plugin hook runs. A valid composition provisions declared dependencies
atomically, starts and tears down in deterministic dependency order, and gives
application code direct typed handles. The graph is control-plane state; it is
not traversed on the normal call path.

The engine remains host-, runtime-, provider-, and domain-neutral. Hosts own the
outer execution model, Plugins own their resources and admission behavior, and
applications keep their business policy outside Kernox.

## Users

- Rust product engineers composing modular monoliths, services, workers, CLIs,
  serverless functions, games, or other application hosts.
- Library authors publishing reusable domain, adapter, or host plugins with
  explicit typed capability contracts.
- Platform engineers exposing existing services through replaceable adapters
  without transferring service authority into Kernox.

## Product boundaries

Kernox is a plugin and capability composition engine. It is not:

- a service locator, ambient global resolver, event bus, broker, ORM, service
  mesh, or deployment control plane;
- a security sandbox or process-isolation boundary for native Plugins, which
  are trusted code sharing the Host process;
- the owner of HTTP, identity, storage, billing, AI, queue, workflow, or other
  business capabilities;
- a reason to turn every function, entity, adapter, or crate into a Plugin; or
- a promise that different domain semantics become reusable merely because
  they share a packaging shape.

Arbitrary native dynamic-library loading and runtime-isolated Plugin formats
are outside the native path. Any future out-of-process or WebAssembly extension
must preserve the same composition semantics without weakening direct native
calls.

## Success and maturity

Kernox is currently a pre-1.0 experimental engine on the `0.1.x` package train.
Local source oracles, a merged change, and a public package are separate facts.
The first production release is admitted only when every node in the product
capability DAG has its required evidence, the public API is reviewed for 1.x
compatibility, the reference applications pass, and the exact package artifacts
are published and read back from the registry.

Product adoption is a separate consumer decision. Kernox earns adoption only
where an independent comparison shows that it reduces composition and lifecycle
entropy without a material steady-state regression.

## Canonical references

| Document | Authority |
| --- | --- |
| This file | Canonical destination, users, product boundaries, success, and maturity |
| [`docs/capabilities.md`](capabilities.md) | Stable `KNX-*` product architecture, prerequisites, and admission oracles |
| [`docs/prd.md`](prd.md) | Detailed KR-* requirements, invariants, non-goals, and release criteria |
| [`docs/specs/20260815T185400Z-runtime-contract.md`](specs/20260815T185400Z-runtime-contract.md) | Resolution, provisioning, lifecycle, scope, and Host semantics |
| [`docs/adr/20260815T185400Z-static-capability-graph.md`](adr/20260815T185400Z-static-capability-graph.md) | Static graph architecture decision |
| [`docs/specs/20260815T185400Z-acceptance.md`](specs/20260815T185400Z-acceptance.md) | Production release claims and falsifiable oracles |
| [`docs/critical-path.md`](critical-path.md) | Risk sequence, redesign triggers, and adoption gate |
