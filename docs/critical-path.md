# Production critical path

This path sequences risk; it does not reduce the destination. No gate is called
an MVP, and no intermediate green state is a production-release claim.

## Gate 0 — Authority and contracts

Exit evidence:

- Product promise, non-goals, architecture decision, runtime semantics, threat
  model, acceptance oracles, license intent, and delivery terminal are durable.
- Technology choices come from current authoritative sources.
- The public repository and contribution path exist.

## Gate 1 — Graph correctness

Exit evidence:

- Validated identifiers, versions, requirements, conflicts, bindings, and
  deterministic topological resolution are implemented.
- Unit, table, and property tests cover insertion-order invariance, ambiguity,
  incompatibility, cycles, conflicts, bounds, and stable diagnostics.
- Graph export and explanation use the same semantic authority as runtime
  resolution.

## Gate 2 — Lifecycle and ownership

Exit evidence:

- Typed provisioning, transactional initialization, lifecycle state machine,
  reverse rollback, idempotent shutdown, cancellation, and task draining work.
- Failure injection tests cover every transition and partial-success boundary.
- A plugin cannot resolve an undeclared capability or publish a descriptor/type
  mismatch through the supported API.

## Gate 3 — Hosts and product paths

Exit evidence:

- Tokio, provider-neutral serverless invocation, CLI, and deterministic test
  hosts are implemented without contaminating core.
- One unchanged domain plugin runs in both long-lived and serverless examples.
- Request-specific state cannot cross warm serverless invocations.
- Graph inspection and conformance tooling operate on versioned artifacts; the
  three-plugin reference application passes the executable conformance oracle.

## Gate 4 — Production hardening

Exit evidence:

- Threat controls are implemented and falsified by negative, property, fuzz,
  concurrency, and failure-injection tests.
- Public API docs, compile-fail examples, compatibility fixtures, structured
  diagnostics, and recovery behavior are complete.
- Benchmarks compare graph construction and direct runtime calls with a
  hand-written Rust baseline; budgets in the acceptance contract hold.
- Dependency, license, advisory, unsafe-code, and secret boundaries are green.

## Gate 5 — Release

Exit evidence:

- Exact candidate passes the single repository verification entrypoint and CI
  on PR and merge-group events.
- Every workflow job runs on one approved Sylphx Platform self-hosted profile;
  the macOS portability lane is retained, while Windows portability remains an
  explicit Platform-owned acceptance residual until an approved profile exists.
- The publishable package set has one version, complete metadata, and a
  topological dependency order; `cargo package --locked --workspace` produces
  the full dry-run artifact set from the locked source.
- API semantic-version checks pass against the admitted predecessor where one
  exists.
- Documentation examples compile from clean consumers.
- Immutable packages are published in dependency order, a provenance receipt
  records the source revision, lockfile, toolchain, and crate checksums, and
  registry readback passes against those exact artifacts.
- Source, CI, merged, package, and adoption states are reported separately.

## Kill or redesign triggers

Stop release and redesign the owning boundary if any remains true:

1. Core contains product/provider-specific policy.
2. A normal call performs graph lookup, serialization, or event dispatch.
3. Adding a simple plugin requires more permanent concepts than direct Rust
   composition without buying lifecycle or reuse proof.
4. The same domain plugin needs host-specific business-code changes.
5. Failure or removal leaves routes, tasks, callbacks, or provisions active.
6. Native plugins are represented as sandboxed or runtime-unload-safe.
7. Versioning forces unrelated applications into lockstep upgrades.
8. Kernox is useful only with Sylphx services.
9. Reference applications show no measurable reduction in repeated composition
   and lifecycle glue.

## First adoption decision

Existing products remain outside scope. Adoption is reconsidered only after the
released engine passes all gates and an independent comparison against direct
Rust composition demonstrates lower total entropy without material runtime
regression.
