# ADR 20260815T185400Z: Static capability graph as the kernel authority

Status: Accepted

## Context

Kernox must let independently owned modules compose products while preserving
Rust performance, explicit dependency direction, deterministic lifecycle, and
serverless compatibility. A literal "everything is a plugin" model risks
turning ordinary code into framework objects, while a runtime event bus or
service locator hides dependencies and taxes every call.

## Decision

Kernox is a graph-backed application kernel.

- Plugin descriptors and explicit product bindings are the composition input.
- Plugin and capability identities are graph nodes; selected provision and
  lifecycle dependencies are directed edges.
- The graph is resolved once before initialization and readiness.
- Startup follows deterministic topological order; teardown follows the exact
  reverse order.
- Provision resolution occurs during initialization. Consumers retain direct
  typed handles; the graph and registry are not an application data plane.
- Static native composition is the default and shares one process/trust
  boundary.
- The Host owns the outer execution model and creates application/invocation
  scopes. Kernox core knows no specific async runtime or transport.
- Commands and queries use direct capability calls. A generic application Event
  Bus is not part of core. Optional local signals and durable integration-event
  adapters remain separate packages/capabilities.

The five public concepts are App, Plugin, Capability, Scope, and Host. A bundle
is ergonomic composition, not another runtime primitive.

## Graph semantics

- A requirement selects one compatible provider by explicit binding or unique
  compatible candidate.
- Optional requirements select zero or one provider.
- Multi requirements select every compatible provider in stable identity order.
- Multiple candidates for a single requirement are ambiguous unless bound.
- Selected provider-to-consumer edges determine lifecycle order.
- Conflicts, duplicate identities, type/descriptor disagreement, incompatible
  versions, self-dependency, cycles, and configured size limits fail closed.
- Resolution and diagnostics are deterministic under descriptor insertion
  permutation.

## Lifecycle semantics

Initialization returns a staged provision set plus lifecycle object. Kernox
validates the staged set against the descriptor before atomic publication.
Initialization or startup failure unwinds completed plugins in reverse order.
Shutdown quiesces, stops, and disposes in reverse order and is idempotent.

Plugin code owns its resources but cannot self-elect global ordering. Official
Host task APIs bind spawned work to the application supervisor and support
cancellation, bounded draining, named leak reports, and fail-closed panic
handling. Individual plugins still close their own admission and resources in
their lifecycle hooks. Serverless correctness cannot depend on shutdown being
observed.

## Rejected alternatives

- **Cargo conventions only:** lowest initial cost but does not standardize
  dependency diagnostics, transactional lifecycle, scoped ownership, host
  portability, or conformance.
- **Type-level dependency DAG:** can move some errors to compilation but creates
  excessive generic/proc-macro complexity, compile-time cost, and poor dynamic
  diagnostics. Admission-time graph validation is the initial authority.
- **Global service locator:** easy wiring but hides dependencies and lets domain
  code retain ambient authority.
- **Core event bus:** creates implicit control flow and undefined ordering,
  transaction, retry, and backpressure semantics.
- **Native dynamic libraries:** Rust's native ABI is not a stable public plugin
  contract and in-process loading is not isolation.
- **Wasm from the first release:** adds ABI, serialization, runtime, and resource
  policy costs before an untrusted or cross-language need is demonstrated.

## Consequences

Kernox pays a bounded composition/startup cost and one explicit Plugin authoring
contract. In exchange, invalid application graphs fail before readiness,
lifecycle ownership is uniform, and runtime hot paths remain ordinary Rust.
Future process or Wasm hosts may reuse descriptor and lifecycle semantics but
must add explicit serialization, permissions, resource, compatibility, and
failure-isolation contracts.

## Verification

The acceptance matrix binds graph properties, lifecycle rollback, scope
closure, host portability, security controls, and performance comparisons to
executable tests and benchmarks.
