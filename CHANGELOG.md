# Changelog

All notable changes are documented here. Kernox follows Semantic Versioning;
descriptor/report schema compatibility is versioned separately where stated.
The project is intentionally pre-1.0 until its engine contracts and adoption
evidence are mature.

## [Unreleased]

### Changed

- Public listing, contributing path, delivery gates, and `GraphBuilder::new` rustdoc no longer sell Kernox as a default company kernel or a production product. Dest remains experimental-engine, not a default dependency.

## [0.1.1]

Not a published release, and no release date is recorded. The first dest
`0.1.x` package-probe candidate exists as tag `v0.1.1` (lightweight tag pushed
2026-09-10, commit `bb4a2e1`), but the tag-gated publisher has not completed
crates.io readback: crates.io serves only `0.0.1` (not yanked) for all seven
packages.

Evidence verified 2026-09-10: release run `34530199204` failed at OIDC
authentication (error: `No Trusted Publishing config found for repository
SylphxAI/kernox`) and skipped the publish/readback step; no GitHub Release
exists for the tag (`releases/tags/v0.1.1` returns 404); `git ls-remote --tags
origin` shows `refs/tags/v0.1.1` = `bb4a2e13807b0d1951c253b0e30941f8008d674b`
with no dereferenced `^{}` line (lightweight); the crates.io versions API
returns only `0.0.1` for `kernox`, `kernox-core`, `kernox-runtime`,
`kernox-host-serverless`, `kernox-host-tokio`, `kernox-testkit`, and
`cargo-kernox`. Immutable tag `v0.1.0` is an older ancestor and is not this
probe; `0.0.1` remains crate-name existence until dest `0.1.x` registry
readback.

### Changed

- Facade crate `kernox` package description now matches the README one-sentence purpose.
- PRD and PROJECT.md now name dest `0.1.x` trusted publishing as the first public package probe; stable 1.x remains refused until the engine is admitted mature.
- Compatibility policy and the delivery critical path now name dest `0.1.x` as the first public package probe and treat published `0.0.1` as crate-name existence, not dest, even when CI compares public APIs against that predecessor.
- README Release state now names dest `0.1.x` as the first public package probe and treats published `0.0.1` as crate-name existence, not dest.
- Acceptance matrix title and README link no longer sell the pre-1.0 engine as Production; dest remains the acceptance matrix, not a production-release claim.

### Added

- Deterministic capability graph with versioned provider resolution, explicit
  bindings, conflicts, cycle diagnostics, hard resource ceilings, and stable
  reports.
- Typed atomic provisioning, declared-only dependency access, transactional
  lifecycle rollback, reverse idempotent shutdown, scopes, and privacy-safe
  lifecycle observations.
- Supervised Tokio tasks, provider-neutral warm serverless invocations,
  inspection CLI, conformance testkit, fuzz target, benchmarks, and one
  host-neutral three-plugin reference application.
- Fail-closed task-panic supervision, bounded graph diagnostics, and
  concurrency regressions for scope closure and long-lived child retention.
- Indexed consumer/capability requirement lookup during initialization, with
  insertion-order coverage and a dedicated scaling benchmark.
- North Star conformance oracle for three-plugin source-attributed applications,
  including clean startup and shutdown proof on the reference app.
- Independent composition-input and graph-report schema versions, with
  fail-closed report readers that reject an unsupported report major,
  reversed lifecycle order, duplicate plugins, and unknown plugin refs.
- CLI products compose the same graph through `AppBuilder` without Tokio; there
  is no `kernox-host-cli` crate.

### Fixed

- Plugin hook and observation-sink unwinds no longer abort remaining lifecycle
  rollback. The executor reports `plugin.hook-panicked` without a panic payload
  and continues reverse cleanup.
- Graph-level verified-application attribution in `kernox-core`, reused by the
  testkit and `cargo kernox check --verified`.
- Compile-fail oracle that `InitializationContext` cannot escape as `'static`.
- Root capability acquisition now fails closed as soon as application shutdown
  begins, before cleanup hooks finish.
- CLI-without-Tokio host contract (KR-006): `AppBuilder` composes without a
  Tokio host crate, and the CLI host crate remains absent.

[Unreleased]: https://github.com/SylphxAI/kernox/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/SylphxAI/kernox/compare/v0.1.0...v0.1.1
