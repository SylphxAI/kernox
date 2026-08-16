# Kernox

## Purpose

Kernox is a graph-backed application kernel for composing Rust products from
typed, lifecycle-safe plugins without adding a runtime framework tax to the
application hot path.

## Product Vision

Any Rust application can be assembled from independently testable plugins,
with explicit capability dependencies, deterministic lifecycle ownership, and
portable host adapters. Kernox supplies composition mechanisms, not business
features. Static native composition is the default; isolated runtime extension
boundaries are added only when their trust or portability need is proven.

## North Star Metric

**Verified applications:** independently released applications that compose at
least three separately owned plugins, pass Kernox conformance, and do not fork
or patch the Kernox core.

Anti-proxies: GitHub stars, crate downloads, plugin count without reuse,
successful compilation without lifecycle proof, and green CI without a usable
application path.

## Goals

- Deliver the complete production-commercial-grade kernel contract defined in
  `docs/prd.md` and its acceptance matrix.
- Prove the same domain plugin unchanged in long-lived and serverless hosts.
- Keep resolved hot-path overhead within the declared benchmark budget against
  direct Rust composition.
- Publish stable documentation, compatibility policy, security posture, and a
  reproducible release path.

## Architecture profile

- Repository lifecycle: `active`
- Task surface: `product-code`, `runtime-implementation`
- Component `kernox-runtime`: role `runtime`, implementation `rust`, no durable
  business effects
- Technology profile: `technology-stack-profile@2026-08-12.1`, digest
  `sha256:183c1ee98c728525d54b039ac77ea3b821d48380bff0cb7a7dc6399bdd7ad89b`
- Applied rule: `backend-role-requirement`
- Cross-runtime RPC, integration-event, and relational-database rules are not
  activated by the current library boundary.

## Delivery

The requested terminal is a public, production-commercial-grade source and
package release. `cargo run -p xtask -- verify` is the repository verification
entrypoint. A local diff, commit, pull request, merge, or green CI run is not a
package-release claim. Release automation separately validates the publishable
package set and dependency order with
`cargo run --locked -p xtask -- release-check`, dry-runs the complete workspace
package graph, and records the
tag, source revision, lockfile digest, toolchain, and crate checksums in an
attested provenance receipt before registry publication and readback.

## Links

| Document | Authority |
| --- | --- |
| [README.md](README.md) | Public entry and quick start |
| [docs/prd.md](docs/prd.md) | Product capabilities and requirements |
| [docs/critical-path.md](docs/critical-path.md) | Delivery gates and kill criteria |
| [docs/adr/](docs/adr/) | Durable architecture decisions |
| [docs/specs/](docs/specs/) | Runtime and acceptance contracts |
| [SECURITY.md](SECURITY.md) | Supported security reporting path |
