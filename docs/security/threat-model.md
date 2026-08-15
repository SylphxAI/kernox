# Public threat model and security design contract

## Subject and evidence state

Subject: Kernox native static composition, host SDKs, graph inspection format,
build/release path, and the future isolated-extension seam. This document is a
design contract until linked implementation tests exist. It intentionally omits
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
| T-02 | Descriptor spoofing or duplicate identity redirects a requirement | Validated stable IDs, versions, unique providers, explicit bindings, fail-closed ambiguity | Unit/property tests over collisions and binding permutations |
| T-03 | Descriptor/type mismatch causes capability confusion | Typed `TypeId` plus declared identity/version validation before atomic publication | Negative provisioning tests and compile-fail fixtures |
| T-04 | Resolver/service-locator escape grants undeclared ambient authority | Borrowed initialization resolver scoped to declared requirements; no global resolver API | Compile-fail and runtime undeclared-resolution tests |
| T-05 | Partial initialization or cleanup failure leaves active resources | Transactional publication, reverse rollback, continued cleanup, composite failure report | Full transition failure-injection model |
| T-06 | Plugin task ignores cancellation and prevents shutdown | Host task scope, bounded drain, leak report; forceful process policy remains Host-owned | Paused-time cancellation and leak fixtures |
| T-07 | Pathological graph/report exhausts CPU or memory | Configurable node/edge/metadata/input limits; linear algorithms; reject before allocation growth where practical | Fuzz/property tests and large-graph benchmarks |
| T-08 | Warm serverless state leaks between callers | Immutable App state, fresh Invocation Scope, no request data in globals through host API | Concurrent distinct-invocation tests |
| T-09 | Panic is mistaken for recoverable plugin isolation | Typed expected failures; document native panic as process failure; isolation only at real boundary | Panic policy tests/documentation and Host recovery tests |
| T-10 | Dependency or action compromise changes release output | Locked dependencies, advisory/license/source checks, immutable action pins, least-privilege CI, provenance-ready release | CI policy, lockfile audit, package dry-run, release readback |
| T-11 | Diagnostics disclose secret/plugin payload data | Allowlisted structured fields only; no arbitrary payload/debug object | Snapshot/negative tests with sentinel secrets |
| T-12 | Runtime extension receives excess host authority | Future explicit WIT/process capability grants, deny by default, resource quotas and versioned ABI | Required before isolated-extension implementation or release |

## Residual risk

- Native plugin compromise equals host-process compromise. The application
  owner accepts that risk by compiling the crate; Kernox cannot accept it on
  their behalf.
- Safe Rust and `unsafe_code = "forbid"` reduce memory-safety risk in owned
  code but do not eliminate compiler, dependency, kernel, or logical defects.
- A task can ignore cooperative cancellation. The Host must enforce its outer
  shutdown/resource policy and report incomplete draining.
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
