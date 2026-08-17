# Changelog

All notable changes are documented here. Kernox follows Semantic Versioning;
descriptor/report schema compatibility is versioned separately where stated.
The project is intentionally pre-1.0 until its engine contracts and adoption
evidence are mature.

## [Unreleased]

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

### Fixed

- Plugin hook and observation-sink unwinds no longer abort remaining lifecycle
  rollback. The executor reports `plugin.hook-panicked` without a panic payload
  and continues reverse cleanup.
- Graph-level verified-application attribution in `kernox-core`, reused by the
  testkit and `cargo kernox check --verified`.
- Compile-fail oracle that `InitializationContext` cannot escape as `'static`.
- Root capability acquisition now fails closed as soon as application shutdown
  begins, before cleanup hooks finish.

[Unreleased]: https://github.com/SylphxAI/kernox/commits/main
