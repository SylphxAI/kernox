# ADR 20260815T210006Z: Typed indexed execution and resumable lifecycle

Status: Accepted

## Context

The unreleased source candidate proves that resolved capability calls can be a
normal Rust call, but it does not yet meet the first public release contract.
Native plugin declarations repeat capability identity and version strings,
dependency acquisition scans the global requirement list, independent plugins
boot serially, and dropping an in-flight lifecycle future loses its progress.
Invocation scopes own identity and admission only; they cannot own typed
request-scoped provisions.

There is no published crate, known external consumer, production store, or
durable business state. The first public release can therefore replace the
candidate atomically. Git history is the only predecessor archive. No runtime
adapter, deprecated alias, dual schema reader, or fallback path is justified.

## Decision

Kernox keeps five public concepts: **App, Plugin, Capability, Scope, and Host**.
The graph remains the control plane; extracted handles remain the data plane.
The candidate is hard-cut to the following one-authority design.

### Compile-time native contracts

`kernox-macros` generates the only supported native authoring evidence:

```rust
#[derive(kernox::Capability)]
#[capability(
    id = "dev.example.clock",
    version = "1.0.0",
    scope = Application,
    interface = dyn Clock
)]
pub struct ClockCapability;

#[derive(kernox::PluginContract)]
#[plugin(id = "dev.example.orders", version = "1.0.0")]
#[provides(OrderServiceCapability)]
#[requires(ClockCapability, version = "^1", cardinality = One)]
pub struct OrdersContract;
```

The macros validate identifier, semantic-version, version-requirement,
duplicate declaration, scope, and cardinality syntax while compiling. They
generate local-slot evidence:

```rust
Provides<C>::SLOT
Requires<C>::SLOT
Requires<C>::Cardinality
```

`Plugin` has an associated `Contract`. Typed initialization and provision sets
admit `require`, `optional`, `all`, and `provide` only when the generated trait
evidence exists and the cardinality and scope match. Raw offers, requirements,
and descriptors remain wire-format implementation details for offline graph
inspection; they are not a second native authoring API.

Capability interfaces may be concrete types or trait objects. Concrete
interfaces retain monomorphized dispatch after extraction; `dyn Trait`
interfaces retain provider substitutability. Kernox does not create two graph
or lifecycle semantics for those dispatch choices.

### Indexed plan and dense registries

Resolution assigns deterministic lexical indices:

```text
PluginId      -> PluginIndex
provided C    -> ProvisionIndex
required C    -> RequirementIndex -> [ProvisionIndex]
```

The immutable `GraphPlan` contains plugin records, contiguous requirements and
provider references, provision slots, and topological waves. Every selected
edge crosses from a lower startup wave to a higher startup wave. Reports and
diagnostics materialize stable string identities from the same plan.

Application and invocation registries are dense provision-slot arrays. A
provision slot transitions exactly once from unset to set. An initialization
context owns its plugin-local indexed dependency table; acquiring `R`
declared dependencies performs `R` local slot accesses, never a global ID scan.
Type erasure is confined to plan construction and handle extraction, where the
generated marker `TypeId` is checked once.

### Persistent, wave-parallel lifecycle

Native plugin hooks return owned, `Send + 'static` futures from shared plugin
instances. Mutable plugin resources therefore use plugin-owned synchronized
state instead of a future borrowing `&mut self`. Kernox can retain in-flight
futures and progress without a self-referential runtime object.

Startup and shutdown are explicit persistent machines:

```text
Resolved -> Initializing(wave) -> Starting(wave) -> Running
                   |                    |
                   +----> RollingBack <-+

Running -> Quiescing(wave) -> Stopping(wave) -> Disposing(wave) -> Terminated
```

`drive(&mut self)` and `shutdown(&mut self)` retain entered/completed state
when their returned future is cancelled. Re-polling resumes the retained
operation; completed hooks are not repeated. A caller that abandons the owning
state object also abandons runtime-neutral async cleanup—Kernox does not claim
an impossible async `Drop`. Official Hosts must retain and drive the state to a
terminal result.

Hooks in one wave are polled concurrently behind a deterministic barrier.
Wave results are validated and batch-committed before the next wave. If several
hooks fail, lexical plugin order chooses the primary failure and preserves the
others. Rollback and shutdown traverse reverse waves; completion order inside a
wave is not a public ordering contract.

Timeout and executor policy belong to the Host. The kernel owns lifecycle
state, deterministic failure selection, rollback, and resumability without
depending on Tokio.

### Typed scopes

`Capability` has an associated sealed scope class. The first release supports
`Application` and `Invocation`:

- application providers may depend only on application providers;
- invocation providers may depend on application providers or an earlier
  invocation wave;
- a parent scope can never depend on a child scope;
- invocation handles carry the invocation lifetime and cannot escape through
  supported APIs.

Invocation providers register typed factories during application startup.
Opening an invocation builds a fresh dense registry by invocation waves;
partial failure rolls it back. Closing runs async teardown in reverse waves.
The explicit invocation session retains progress across cancellation. `Drop`
closes admission as a fail-safe but does not claim async teardown completed.

Task ownership remains a Host resource, not a third capability scope.

### Host hard cuts

- Tokio task futures return a typed `TaskResult`. Normal failures no longer
  require panic or out-of-band signalling. Forced abort waits until tracked
  futures have actually been destroyed before plugin stop completes.
- Serverless handlers use `AsyncFnOnce`; callers do not allocate a boxed
  handler future per invocation. Capacity is released only after scope teardown
  reaches a terminal state.
- Observation failures never prevent lifecycle cleanup. Native plugins and
  observers remain trusted in-process code, not a sandbox.

### Resource and performance contract

Graph limits cover plugins, declarations, edges, bindings, and conflicts.
Tests enforce index bounds and linear acquisition work independently of noisy
wall-clock timing. Benchmarks cover resolve, real boot, shutdown, invocation,
allocations, and direct calls for sparse and dense 10/100/1,000-plugin graphs.

The steady-state contract remains: an extracted handle call performs no graph
lookup, registry lookup, serialization, event dispatch, or allocation. Its
paired 95% upper overhead ratio must not exceed 1.02 against the equivalent
direct Rust dispatch on the declared reference environment.

## Cutover and data preservation

The cut surface is the native API, graph wire fixture, examples, docs, tests,
CLI output, and package artifacts. There is no database or persisted runtime
record to migrate or backfill. The truthful zero-loss oracle is:

1. re-query registries and known consumers immediately before release;
2. rewrite every repository-owned caller and fixture to the final contract;
3. prove equivalent reference application outcomes in long-lived and
   serverless Hosts;
4. prove the predecessor symbols and schema reader are absent from exports,
   builds, examples, and current documentation; and
5. publish only the exact admitted source and read it back through clean
   registry-only consumers.

No empty migration, fake backfill receipt, or compatibility code is created.

## Acceptance and release

The exact candidate must pass formatting, clippy, MSRV, all features, doctests,
compile-pass/fail UI cases, property and lifecycle-model tests, cross-platform
tests, Miri where supported, fuzzing, mutation testing with workspace consumer
tests, performance contracts, dependency/license/advisory policy, package
inspection, and both reference applications before merge-queue admission.

The seven crates publish in dependency order. Registry publication is not
transactional; an interrupted release must forward-complete the identical tag
and checksums before any GitHub Release or announcement. Completion requires
checksums, licenses, source revision, sparse-index metadata, docs, installed
CLI version, and clean consumer readback for every package.

## Rejected alternatives

- **Full type-level global DAG:** rejected because compile-time and diagnostic
  cost grows with the whole application and prevents dynamic provider
  selection. Rust proves each local contract; the graph resolves composition.
- **Runtime service locator or Event Bus:** rejected because it hides
  dependencies and taxes the data plane.
- **Separate static and dynamic kernels:** rejected because concrete and trait
  object interfaces already select dispatch without duplicating semantics.
- **Serial lifecycle for determinism:** rejected because deterministic waves
  and failure selection preserve correctness without summing independent I/O.
- **Compatibility aliases or schema dual-read:** rejected because no released
  consumer exists and every retained predecessor would become a second public
  authority.

## Consequences

Plugin implementations use interior ownership suitable for concurrent Hosts,
and application boot becomes an explicit driven lifecycle rather than one
consuming convenience future. This is a deliberate first-release API break.
In return, invalid local composition moves to compile time, startup work is
indexed and wave-parallel, cancellation is recoverable, request-scoped
resources have an owned lifecycle, and the hot path remains ordinary Rust.
