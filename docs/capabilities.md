# Kernox identity graph

**Status:** Identity registry. Not live proof.
**Scope:** Kernox — deterministic capability graph and lifecycle kernel.
**Cite:** the **ID** column.

The destination is [`vision.md`](vision.md). This file is the identity graph:
durable names, one fate each, truth-edges, and `Done when` oracles.

Shape: `ID | Identity | Fate | Depends on | Done when`. One colloquial name
has one fate (`live`, `dead`, or `rename-to:<ID>`). Fate is destination, not
status. Current work and delivery state belong to the product pull request,
not this graph.

Field law in `kernox-core`, `kernox-runtime`, `kernox-host-*`, `cargo-kernox`,
`fixtures/`, `docs/prd.md`, and `docs/compatibility.md` is evidence toward
these oracles. If those conflict with this graph or [`vision.md`](vision.md),
they are dest-minus-evidence, not an override of destination.

```text
ID | Identity | Fate | Depends on | Done when
```

## Graph

| ID | Identity | Fate | Depends on | Done when |
| --- | --- | --- | --- | --- |
| KR-CAP | Deterministic capability DAG + validation | live | — | Plugin and capability identifiers, semver + requirements, provides/requires/conflicts, bounded metadata validated; provider selection, bindings, optional/multi requirements, cycles, and startup/teardown order are deterministic and explainable; `cargo kernox check` / `graph --format dot` with fixtures holds. |
| KR-PROVISION | Typed provisioning + injection | live | KR-CAP | Plugins publish typed handles atomically during initialize; consumers resolve only declared capabilities; type/descriptor disagreement rejected; resolver never escapes the initialization borrow. |
| KR-LIFECYCLE | Transactional lifecycle (compose → dispose) | live | KR-PROVISION | Compose, validate, initialize, start, ready, quiesce, stop, dispose with typed state; invalid transitions fail; partial init/start rolls back in reverse dependency order and does not publish partial provisions. |
| KR-HOST | Host SDK (Tokio + warm serverless + CLI composition + test host) | live | KR-LIFECYCLE | Long-lived Tokio host (`kernox-host-tokio`), provider-neutral warm serverless with fresh scope per invocation (`kernox-host-serverless`), and deterministic test host (`kernox-testkit`) run the same graph. CLI products compose that graph through `AppBuilder` without Tokio; there is no `kernox-host-cli` crate. `cargo-kernox` is KR-TOOLING inspection, not a host. `cargo run -p kernox-example-*` examples hold. |
| KR-OBSERVE | Diagnostics + shutdown + supervision | live | KR-HOST | Every graph/lifecycle failure has stable tag, structured context, human explanation; reverse-order idempotent shutdown; supervised Tokio tasks with cancellation, panic fail-closed, bounded drain, leak naming, privacy-safe observations. |
| KR-TOOLING | Inspection + conformance tooling | live | KR-CAP | Versioned graph description export, `cargo-kernox` validation/rendering, verified-application source attribution, reference fixtures, and `kernox-testkit` conformance hold. `cargo kernox check --verified` on the reference fixtures holds. |
| KR-COMPAT | Compatibility and extension ladder | live | KR-CAP, KR-HOST | Host capability negotiation fails closed before plugin initialization on missing, duplicate, or incompatible host properties. Composition and graph-report readers reject unsupported schema majors. Native dynamic libraries are not a supported extension mechanism. WebAssembly Component and out-of-process plugins remain unadmitted until an accepted ABI/WIT, capability-grant, resource, and migration contract exists; they must not weaken the native static path before that contract. `docs/compatibility.md` is the compatibility policy. Public-API comparison against published predecessors, when they exist, is evidence, not a substitute for those fail-closed readers. |
| KR-PUBLISH | Dest `0.1.x` crates.io probe | live | KR-HOST, KR-OBSERVE, KR-TOOLING, KR-COMPAT | A tagged `0.1.x` workspace version that is an ancestor of `main` is published by the tag-gated `.github/workflows/release.yml` writer using OIDC trusted publishing (environment `crates-io`) for `kernox-core`, `kernox-runtime`, `kernox-host-serverless`, `kernox-host-tokio`, `kernox-testkit`, `kernox`, and `cargo-kernox`. `https://crates.io/crates/kernox` serves that version with registry checksum equal to that tag's `release-manifest.json`; `cargo add kernox@<0.1.x>` resolves it. `0.0.1` is crate-name existence bootstrap, not this oracle, and is yanked after dest `0.1.x` readback. Stable 1.x remains refused until the engine is admitted mature. |

Edges are hard prerequisites. A green `cargo test` or version bump does not
close a node whose fixture or example fails.

Verified independently-owned released applications are the North Star Metric
in [`vision.md`](vision.md), not a row here.

## Release boundary (GOV-017)

Declared per [ADR-030](https://github.com/SylphxAI/owner/blob/main/decisions/ADR-030-RELEASE-CONTROL-PLANE.md)
and [GOV-017](https://github.com/SylphxAI/owner/blob/main/runbook/GOVERNANCE-AUDIT-2026-08-28.md),
grounded in the rows above and `.github/workflows/release.yml`. This is dest,
not live proof.

- **Public probe.** Dest is a tagged `0.1.x` workspace version whose crates.io
  registry checksum equals the artifact checksum recorded in that tag's
  `release-manifest.json` provenance receipt; `cargo add kernox@<0.1.x>`
  resolves it from the registry. `https://crates.io/crates/kernox` serving
  `0.0.1` is crate-name existence bootstrap, not this probe. After dest
  `0.1.x` readback, `0.0.1` is yanked.
- **Owned writers.** The tag-gated `release.yml` is the sole release-intent
  and publishing writer: tag must equal the workspace version and be an
  ancestor of `main`, stable 1.x publication is refused, semver is checked
  against published predecessors, packages are built `--locked`, a
  `release-manifest.json` + `.sha256` provenance receipt is written, build
  provenance is attested, and publication proceeds in dependency order with
  registry readback via OIDC trusted publishing (environment `crates-io`).
  It also owns the Cargo workspace manifests and `cargo-kernox`. No
  migration writers exist. API-token publish is not the dest writer.
- **Owned post-readback actions.** After the dest `0.1.x` registry readback
  holds, the same tag-gated writer yanks the `0.0.1` crate-name-existence
  bootstrap versions, and it owns the maintenance action that registers and
  reads back the crates.io trusted-publisher configs for the release crates.
  Registration uses a registry API token with the crates.io
  `trusted-publishing` scope and is never a publish path; the yank uses a
  registry API token with the `yank` scope and refuses to act before the
  readback above is re-proved from the registry itself: for every release
  crate the registry API must serve the tag's version as a not-yanked record,
  the crate index must serve that version with the same checksum, the bytes
  served by the registry must hash to that checksum, a locked rebuild of the
  tagged clean source must be byte-identical to them, and those bytes must
  record the tag's commit as their source revision.
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
  or provenance attestation, must not treat a green `cargo test` or a
  version bump as release evidence (this file's edges rule), must not treat
  `0.0.1` crate-name existence as dest, and must not treat API-token publish
  as the dest writer. No second publish writer.
- **Forbidden yank.** The `0.0.1` bootstrap must not be yanked before the dest
  `0.1.x` readback is re-proved from the registry, and a yank must not be
  reported before the registry serves `yanked: true`.
