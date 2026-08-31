# Kernox identity graph

**Status:** Identity registry. Not live proof.
**Scope:** Kernox — deterministic capability graph and lifecycle kernel.
**Cite:** the **ID** column.

This file is the identity graph. It is not a PRD, ADR index, or live grade. Destination stays in [`vision.md`](vision.md). Field law stays in `kernox-core`, `kernox-runtime`, `kernox-host-*`, `cargo-kernox`, `fixtures/`, and `docs/prd.md`. If this file conflicts with those, this file is wrong.

```text
ID | Identity | Fate | Depends on | Done when
```

## Graph

| ID | Identity | Fate | Depends on | Done when |
| --- | --- | --- | --- | --- |
| KR-CAP | Deterministic capability DAG + validation | live | — | Plugin and capability identifiers, semver + requirements, provides/requires/conflicts, bounded metadata validated; provider selection, bindings, optional/multi requirements, cycles, and startup/teardown order are deterministic and explainable; `cargo kernox check` / `graph --format dot` with fixtures holds. |
| KR-PROVISION | Typed provisioning + injection | live | KR-CAP | Plugins publish typed handles atomically during initialize; consumers resolve only declared capabilities; type/descriptor disagreement rejected; resolver never escapes the initialization borrow. |
| KR-LIFECYCLE | Transactional lifecycle (compose → dispose) | live | KR-PROVISION | Compose, validate, initialize, start, ready, quiesce, stop, dispose with typed state; invalid transitions fail; partial init/start rolls back in reverse dependency order and does not publish partial provisions. |
| KR-HOST | Host SDK (Tokio + warm serverless) | live | KR-LIFECYCLE | Long-lived Tokio host, provider-neutral warm serverless with fresh scope per invocation, CLI and deterministic test host run the same graph; `cargo run -p kernox-example-*` examples hold. |
| KR-OBSERVE | Diagnostics + shutdown + supervision | live | KR-HOST | Every graph/lifecycle failure has stable tag, structured context, human explanation; reverse-order idempotent shutdown; supervised Tokio tasks with cancellation, panic fail-closed, bounded drain, leak naming, privacy-safe observations. |
| KR-TOOLING | Inspection + conformance tooling | live | KR-CAP | Versioned graph description export, `cargo-kernox` validation/rendering, verified-application source attribution, reference fixtures, and `kernox-testkit` conformance hold. |

Edges are hard prerequisites. A green `cargo test` or version bump does not close a node whose fixture or example fails.

## Release boundary (GOV-017)

Declared per [ADR-030](https://github.com/SylphxAI/owner/blob/main/decisions/ADR-030-RELEASE-CONTROL-PLANE.md)
and [GOV-017](https://github.com/SylphxAI/owner/blob/main/runbook/GOVERNANCE-AUDIT-2026-08-28.md),
grounded in the rows above and `.github/workflows/release.yml`. This is dest,
not live proof.

- **Public probe.** `https://crates.io/crates/kernox` serves a published
  version whose registry checksum equals the artifact checksum recorded in
  that tag's `release-manifest.json` provenance receipt; `cargo add
  kernox@<version>` resolves it from the registry.
- **Owned writers.** The tag-gated `release.yml` is the sole release-intent
  and publishing writer: tag must equal the workspace version and be an
  ancestor of `main`, stable 1.x publication is refused, semver is checked
  against published predecessors, packages are built `--locked`, a
  `release-manifest.json` + `.sha256` provenance receipt is written, build
  provenance is attested, and publication proceeds in dependency order with
  registry readback. It also owns the Cargo workspace manifests and
  `cargo-kernox`. No migration writers exist.
- **Consumed receipts.** crates.io registry readback (index status, version
  identity, checksum, not-yanked) is the publication truth it consumes;
  GitHub build-provenance attestation receipts bind artifacts to the tagged
  source revision; the `origin/main` ancestry check receipt binds the tag to
  landed source.
- **Runtime effects.** None beyond consumers: a kernel/library that runs
  only inside consuming hosts, examples, and test hosts
  (`kernox-host-*`, `kernox-example-*`); it deploys nothing.
- **Forbidden writes.** Stable 1.x publication is intentionally refused until
  the engine is admitted mature; it must not publish a version whose source
  revision is not the tagged `main` commit, must not skip registry readback
  or provenance attestation, and must not treat a green `cargo test` or a
  version bump as release evidence (this file's edges rule). No second
  publish writer.
