# Kernox capability architecture

This file is the canonical product capability DAG for the destination owned by
[`docs/vision.md`](vision.md). It includes every Kernox responsibility required
to reach that destination whether or not the current source revision has
implemented or admitted it. Implementation evidence is subordinate to the DAG;
missing evidence never removes a responsibility from the product.

| Home | Owns |
| --- | --- |
| [`docs/vision.md`](vision.md) | Canonical destination, users, boundaries, and maturity |
| This file | Stable product capabilities, prerequisite edges, and admission oracles |
| [`docs/prd.md`](prd.md) | Detailed KR-* requirements, invariants, non-goals, and release criteria |
| [`docs/specs/20260815T185400Z-runtime-contract.md`](specs/20260815T185400Z-runtime-contract.md) | Identifier, resolution, provision, lifecycle, scope, and Host semantics |
| [`docs/specs/20260815T185400Z-acceptance.md`](specs/20260815T185400Z-acceptance.md) | Detailed release claims and validation lanes |
| [`docs/adr/20260815T185400Z-static-capability-graph.md`](adr/20260815T185400Z-static-capability-graph.md) | Why the static graph is the kernel authority |

## How to read the DAG

- A `KNX-*` node is one stable product responsibility.
- A directed edge `A -> B` means B cannot be admitted unless A is true.
- An admission oracle states the falsifiable terminal for that responsibility.
  It does not assert that the current revision passes it.
- Source, CI, merge, package publication, registry readback, product adoption,
  and live use remain separate evidence layers.
- `KNX-EX-*` labels identify revision-local evidence compositions below. They
  are not product capability nodes.
- Native Plugins are trusted in-process code. Nothing in this graph is a
  sandbox or a service locator.

## Product capability DAG

Edges point from prerequisite to dependent. Every capability converges on
`KNX-PRODUCT`, the first admitted Kernox product release.

```text
KNX-ID -> KNX-GRAPH
KNX-GRAPH -> KNX-REPORT -> KNX-INSPECT
KNX-GRAPH -> KNX-ATTRIB -> KNX-CONFORM
KNX-ATTRIB -> KNX-INSPECT
KNX-GRAPH -> KNX-CORE-INDEP
KNX-GRAPH -> KNX-FUZZ
KNX-GRAPH -> KNX-PROVISION -> KNX-HOTPATH
KNX-PROVISION -> KNX-LIFECYCLE
KNX-LIFECYCLE -> KNX-ADMISSION
KNX-LIFECYCLE -> KNX-OBS
KNX-LIFECYCLE -> KNX-TESTKIT -> KNX-CONFORM
KNX-LIFECYCLE -> KNX-SCOPE
KNX-SCOPE -> KNX-HOST-TOKIO
KNX-PROVISION -> KNX-HOST-TOKIO
KNX-SCOPE -> KNX-HOST-SERVERLESS
KNX-REPORT -> KNX-COMPAT
KNX-LIFECYCLE -> KNX-COMPAT
KNX-COMPAT -> KNX-EXTENSION
KNX-PROVISION -> KNX-EXTENSION
KNX-HOTPATH -> KNX-EXTENSION
KNX-INSPECT -> KNX-RELEASE
KNX-CONFORM -> KNX-RELEASE
KNX-ADMISSION -> KNX-RELEASE
KNX-OBS -> KNX-RELEASE
KNX-HOST-TOKIO -> KNX-RELEASE
KNX-HOST-SERVERLESS -> KNX-RELEASE
KNX-HOTPATH -> KNX-RELEASE
KNX-CORE-INDEP -> KNX-RELEASE
KNX-FUZZ -> KNX-RELEASE
KNX-EXTENSION -> KNX-RELEASE
KNX-RELEASE -> KNX-PRODUCT
```

## Capability register

| ID | Product responsibility | Depends on | Admission oracle |
| --- | --- | --- | --- |
| `KNX-ID` | Stable validated Plugin and Capability identities, semantic versions, requirements, offers, conflicts, attribution, and bounded metadata | — | Malformed, duplicate, oversized, or self-conflicting descriptors fail with typed stable tags |
| `KNX-GRAPH` | Pure deterministic provider selection, explicit bindings, cardinalities, conflicts, hard limits, cycles, and dependency order | `KNX-ID` | Equivalent descriptor permutations resolve to the same graph and invalid compositions fail before any Plugin hook runs |
| `KNX-REPORT` | Independently versioned graph projection with selected edges, diagnostics, startup order, and exact reverse teardown order | `KNX-GRAPH` | Semantic round trips are stable; unsupported majors and inconsistent or referentially invalid projections fail closed |
| `KNX-ATTRIB` | Verified-application source attribution over the resolved graph | `KNX-GRAPH` | Insufficient, missing, or duplicate source-package attribution fails with the same tags in core inspection and conformance |
| `KNX-INSPECT` | Bounded CLI validation and DOT/JSON rendering from the same graph authority | `KNX-REPORT`, `KNX-ATTRIB` | CLI and core agree on valid, invalid, and verified compositions; untrusted input exceeds no declared bound |
| `KNX-PROVISION` | Declared-only typed dependency access and atomic publication of complete provisions | `KNX-GRAPH` | Undeclared, wrong-cardinality, missing, duplicate, version-, or type-mismatched access fails before readiness; the resolver borrow cannot escape |
| `KNX-LIFECYCLE` | Transactional initialize/start, deterministic readiness, reverse rollback, quiesce/stop/dispose, unwind isolation, and idempotent shutdown | `KNX-PROVISION` | Failure injection across every phase preserves the primary failure, continues reverse cleanup, publishes no partial state, and repeats no terminal effect |
| `KNX-ADMISSION` | One explicit boundary that closes new application work and root capability acquisition before cleanup | `KNX-LIFECYCLE` | New work fails once quiesce begins; previously acquired direct handles remain a documented non-revocable residual |
| `KNX-SCOPE` | Host-neutral application/invocation identity, parentage, closure, and lifetime containment | `KNX-LIFECYCLE` | Concurrent invocations receive distinct children, closed parents reject registration, and supported APIs cannot leak invocation views |
| `KNX-OBS` | Privacy-safe typed lifecycle observations without a telemetry SDK in core | `KNX-LIFECYCLE` | Stable identities, phases, outcomes, durations, and error tags are emitted without arbitrary payloads; a sink failure cannot abort lifecycle cleanup |
| `KNX-TESTKIT` | Deterministic, duration-free lifecycle recording and typed fault injection | `KNX-LIFECYCLE` | Tests can falsify order and every partial-success boundary without network, process signals, or wall-clock sleeps |
| `KNX-CONFORM` | One application conformance oracle over a resolved graph, attribution, real startup, and clean shutdown | `KNX-ATTRIB`, `KNX-TESTKIT` | Too-small, unattributed, or dirty-shutdown applications cannot produce a conformance report |
| `KNX-HOST-TOKIO` | Official long-lived Tokio task supervision with named admission, cancellation, bounded drain, forced abort, and fail-closed panic reporting | `KNX-SCOPE`, `KNX-PROVISION` | Cooperative, stubborn, panicking, over-capacity, and missing-runtime tasks reach typed deterministic terminals without surviving shutdown |
| `KNX-HOST-SERVERLESS` | Provider-neutral warm application reuse with isolated invocation scopes, bounded concurrency, and explicit shutdown admission | `KNX-SCOPE` | Concurrent calls share no request state, handler failure leaks no invocation, and post-shutdown admission fails |
| `KNX-HOTPATH` | Direct typed application handles after readiness, with no graph, registry, serialization, or event hop | `KNX-PROVISION` | API-shape review and representative benchmarks show ordinary calls remain direct and within the accepted steady-state budget |
| `KNX-CORE-INDEP` | Core independence from Hosts, async runtimes, transports, providers, telemetry SDKs, I/O, and product-domain policy | `KNX-GRAPH` | Minimal-feature compilation and dependency-graph checks reject any forbidden runtime, Host, or product edge |
| `KNX-FUZZ` | Bounded fail-closed handling of untrusted composition input and pathological graphs | `KNX-GRAPH` | Property and fuzz lanes cover parser bounds, graph limits, and adversarial shapes without panic or unbounded work |
| `KNX-COMPAT` | Source, capability, schema, diagnostic, deprecation, and Host-capability compatibility across releases | `KNX-REPORT`, `KNX-LIFECYCLE` | Public API comparison uses the admitted predecessor; old supported fixtures remain readable, unsupported majors fail closed, deprecations retain a replacement window, and unmet Host properties fail before readiness |
| `KNX-EXTENSION` | Native-first extension ladder whose future process or WebAssembly boundaries preserve Kernox semantics without weakening the native path | `KNX-COMPAT`, `KNX-PROVISION`, `KNX-HOTPATH` | The ladder names native static composition as the first rung and keeps it direct; an alternate rung is optional, but cannot be admitted until its ABI/WIT, grants, resources, migration, failure isolation, and conformance are executable and versioned |
| `KNX-RELEASE` | Reproducible commercial release discipline for the complete package train | `KNX-INSPECT`, `KNX-CONFORM`, `KNX-ADMISSION`, `KNX-OBS`, `KNX-HOST-TOKIO`, `KNX-HOST-SERVERLESS`, `KNX-HOTPATH`, `KNX-CORE-INDEP`, `KNX-FUZZ`, `KNX-EXTENSION` | One exact tagged SHA passes locked verification, compatibility, docs, security, dependency/license/advisory, fuzz, benchmark, package, provenance, and reference paths; every immutable package is then published in dependency order and registry readback matches its exact name, version, non-yanked state, and package checksum |
| `KNX-PRODUCT` | Terminal first production release of the Kernox destination | `KNX-RELEASE` | One immutable receipt binds every upstream oracle to the same source SHA, lockfile, toolchain, package set, checksums, and successful registry readback; no local, PR, merge, tag, dry-run, or partial publication can substitute |

## Requirements traceability

| PRD requirement | Capability ownership |
| --- | --- |
| KR-001 | `KNX-ID` |
| KR-002 | `KNX-GRAPH`, `KNX-REPORT` |
| KR-003 | `KNX-PROVISION`, `KNX-HOTPATH` |
| KR-004 | `KNX-LIFECYCLE`, `KNX-ADMISSION` |
| KR-005 | `KNX-SCOPE`, `KNX-HOST-TOKIO` |
| KR-006 | `KNX-HOST-TOKIO`, `KNX-HOST-SERVERLESS`, `KNX-TESTKIT`, `KNX-INSPECT` |
| KR-007 | `KNX-REPORT`, `KNX-OBS` |
| KR-008 | `KNX-ATTRIB`, `KNX-INSPECT`, `KNX-TESTKIT`, `KNX-CONFORM` |
| KR-009 | `KNX-COMPAT`, `KNX-EXTENSION` |
| KR-010 | `KNX-RELEASE`, `KNX-PRODUCT` |

## Revision-local implementation evidence

This section records useful evidence in the current source tree. It does not
define the product DAG and cannot turn an unmet destination oracle into an
absent responsibility.

### Commit-build evidence

`cargo run --locked -p xtask -- verify` exercises the currently implemented
identity, graph, provisioning, lifecycle, scope, observation, testkit, Hosts,
inspection, conformance, direct-call, core-boundary, dependency-policy,
advisory, package, and reference-application paths. Extended fuzz, mutation,
performance-distribution, semantic-version, publication, and registry-readback
lanes remain separate evidence.

| Area | Current executable evidence |
| --- | --- |
| Deterministic graph | `cargo test --locked -p kernox-core --all-features graph` |
| Typed provisioning and lifecycle | `cargo test --locked -p kernox-runtime --all-features --test lifecycle` |
| Tokio supervision | `cargo test --locked -p kernox-host-tokio --all-features --test supervision` |
| Warm serverless isolation | `cargo test --locked -p kernox-host-serverless --all-features --test invocations` |
| Conformance | `cargo test --locked -p kernox-testkit --all-features --test conformance` |
| Inspection | `cargo run --locked -p cargo-kernox -- kernox check fixtures/compositions/verified.json --verified` |
| Release source checks | `cargo run --locked -p xtask -- release-check --version <workspace-version>` |
| Registry terminal mechanism | `.github/workflows/release.yml` publishes the package DAG and compares crates.io name, version, yanked state, and checksum with the built artifacts |

Schema and resource constants on this source revision:

| Contract | Value |
| --- | --- |
| `COMPOSITION_SCHEMA_VERSION` | `1` |
| `GRAPH_REPORT_SCHEMA_VERSION` | `1` |
| Default `GraphLimits` | 4,096 Plugins / 512 declarations per Plugin / 131,072 edges |
| Absolute ceilings | 65,536 / 4,096 / 1,048,576 |
| `MINIMUM_VERIFIED_PLUGINS` | `3` |
| `cargo kernox` input bound | 16 MiB |
| Fuzz target input bound | 1 MiB |

### Evidence compositions

| Evidence ID | Composition | What it exercises |
| --- | --- | --- |
| `KNX-EX-ORDER` | `examples/order-app` | One three-Plugin domain graph used unchanged by long-lived, serverless, and conformance paths |
| `KNX-EX-CHECKOUT` | `examples/checkout-app` | An explicit binding selects one of two compatible payment providers |
| `KNX-EX-WORKER` | `examples/worker-app` | A worker requires the official Tokio task capability and drains on shutdown |
| `KNX-EX-CONSUMER` | `fixtures/clean-consumer` | An out-of-workspace typed consumer exercises direct, workload, serverless, `all`, and `optional` paths |

### Destination evidence frontier

- `KNX-COMPAT` remains in the DAG while stable 1.x compatibility and complete
  Host-capability negotiation lack admitted executable evidence.
- `KNX-EXTENSION` currently admits only the native static rung; no
  out-of-process or WebAssembly implementation is claimed or required merely to
  name the ladder. Its pre-admission contract binds any future rung.
- `KNX-RELEASE` remains in the DAG while release automation has no admitted
  production registry-readback receipt for this product terminal.
- `KNX-PRODUCT` is therefore not admitted. A source pass, green CI, merge,
  tag, package dry-run, or partial registry publication cannot close it.
