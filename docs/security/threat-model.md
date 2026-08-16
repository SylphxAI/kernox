# Public threat model and security design contract

## Subject and evidence state

Subject: Kernox native static composition, host SDKs, graph inspection format,
build/release path, and the future isolated-extension seam. The native/static
controls have executable evidence in this repository; future isolated-extension
controls remain design requirements. This document intentionally omits
exploit-operational details that do not improve public assurance.

Intended users are Rust application and plugin authors. Native plugin code is
selected by the application build and is trusted with the process privileges.
Kernox is not a sandbox, authorization service, or secret store.

## Objectives and assets

- Preserve capability selection and lifecycle integrity.
- Prevent supported APIs from granting undeclared ambient capability access.
- Contain malformed/untrusted inspection input within explicit CPU, memory, and
  size limits.
- Prevent partial initialization, orphaned tasks, and misleading readiness.
- Protect host secrets and request-specific state from diagnostics and warm
  invocation leakage.
- Preserve source, dependency, release, and package integrity.

Unacceptable outcomes include serving after invalid composition, capability
type confusion, lifecycle cleanup being silently skipped, cross-invocation data
leakage, unbounded graph resource consumption, secret disclosure, and claiming
isolation that does not exist.

## Components and trust boundaries

```text
application author (trusted composition)
  -> Cargo and dependency supply chain
  -> native Plugin code (same process and privileges)
  -> Kernox graph / registry / lifecycle
  -> Host adapter
  -> external requests, signals, and services

untrusted graph-report bytes
  -> bounded CLI/parser boundary
  -> pure validation/rendering only

future untrusted extension
  -> process or Wasm boundary (not native Plugin loading)
```

The Cargo/package boundary, external-input boundary, Host/Plugin capability
grant, release automation, and future isolated runtime are separate trust
boundaries. A crate boundary alone is not isolation.

## Threat register

| ID | Path and consequence | Designed control | Verification |
| --- | --- | --- | --- |
| T-01 | Malicious native Plugin reads memory, secrets, or performs arbitrary effects | Declare native Plugins trusted; no sandbox claim; use process/Wasm for untrusted code | Documentation negative review and isolated-extension acceptance tests when implemented |
| T-02 | Descriptor spoofing or duplicate identity redirects a requirement | Validated stable IDs, versions, unique providers, explicit bindings, fail-closed ambiguity | `kernox-core` unit/property tests over collisions, bindings, and insertion order |
| T-03 | Descriptor/type mismatch causes capability confusion | Typed `TypeId` plus declared identity/version validation before atomic publication | `kernox-runtime/tests/lifecycle.rs` negative provisioning/type tests |
| T-04 | Resolver/service-locator escape grants undeclared ambient authority | Borrowed initialization resolver scoped to declared requirements; no global resolver API | Public API surface review, scope compile-fail doctest, and runtime undeclared-access test |
| T-05 | Partial initialization or cleanup failure leaves active resources | Transactional publication, reverse rollback, continued cleanup after hook or observation-sink unwinds, composite failure report | Lifecycle failure injection, hook-unwind regressions in `kernox-runtime/tests/lifecycle.rs`, and `kernox-testkit` probe tests |
| T-06 | Plugin task ignores cancellation, panics silently, or prevents shutdown | Host task scope, panic capture without payload retention, fail-closed admission, bounded drain, format-safe named report, forced task abort | Paused-time cooperative/stubborn/panicking task fixtures and invalid-label tests in `kernox-host-tokio` |
| T-07 | Pathological graph/report exhausts CPU or memory | File, node, declaration, and edge hard ceilings; bounded parser entry; iterative cycle search | Fuzz target, property tests, CLI oversize test, and 10/100/1,000-plugin benchmarks |
| T-08 | Warm serverless state leaks between callers | Immutable App provisions, fresh Invocation Scope, unique IDs, closed-scope deregistration, no request globals in host API | 64-way concurrent invocation test and 10,000-scope retention regression |
| T-09 | Panic is mistaken for recoverable plugin isolation | Typed expected failures; hook unwind is converted to `plugin.hook-panicked` without a payload only so rollback can continue; document native panic as process failure; isolation only at a real process or Wasm boundary | Panic policy tests/documentation, hook-unwind lifecycle tests, and Host recovery tests |
| T-10 | Dependency or action compromise changes release output | Locked dependencies, advisory/license/source checks, immutable action pins, least-privilege CI, OIDC temporary release token | `xtask`, `deny.toml`, pinned workflows, package dry-run, and release registry readback |
| T-11 | Diagnostics disclose secret/plugin payload data | Lifecycle observations contain only plugin/scope/phase/outcome/duration; plugin messages remain in returned failures | Public observation type shape plus recorder snapshots |
| T-12 | Runtime extension receives excess host authority | Future explicit WIT/process capability grants, deny by default, resource quotas and versioned ABI | Required before isolated-extension implementation or release |

## Residual risk

- Native plugin compromise equals host-process compromise. The application
  owner accepts that risk by compiling the crate; Kernox cannot accept it on
  their behalf.
- Safe Rust and `unsafe_code = "forbid"` reduce memory-safety risk in owned
  code but do not eliminate compiler, dependency, kernel, or logical defects.
- A task can ignore cooperative cancellation. The Host must enforce its outer
  shutdown/resource policy. The Tokio adapter reports the task by bounded name
  and aborts it after the configured drain budget.
- The Tokio adapter catches a task unwind only to record failure, close task
  admission, and cancel peers. This is supervision, not an isolation boundary;
  process-wide panic policy and memory corruption remain outside its guarantees.
- The runtime likewise converts a plugin-hook or observation-sink unwind into a
  typed lifecycle failure or a discarded sink fault so remaining cleanup can
  run. That does not contain memory corruption, `panic=abort`, or Drop bombs,
  and it does not make native plugins a sandbox.
- Direct `Arc` handles acquired before shutdown are not revocable without an
  indirection tax. Lifecycle order closes owned work, but callers must stop
  using retained handles when the application begins shutdown.
- Supply-chain scans cannot prove a dependency benign; dependency minimization,
  review, locked resolution, and release provenance remain layered controls.

Review is required when native dynamic loading, untrusted plugins, filesystem or
network grants, cross-language ABI, remote registry installation, privileged
Host operations, or a new release credential path is introduced.

## Publication and retention

This is the public minimum threat model. Reports with exploitable unpublished
details are confidential security artifacts, shared only with maintainers via
the private reporting path in `SECURITY.md` and retained only as needed for
remediation and disclosure coordination.
