# Kernox

Kernox is an experimental embeddable Rust engine that composes a host and trusted in-process plugins into one deterministic capability graph.

- Ordinary: `none` — experimental engine; there is no public customer website.
- Preview: `none` — there is no admitted product-owned preview or dogfood web host.
- Vision: [`docs/vision.md`](docs/vision.md)
- Capabilities: [`docs/capabilities.md`](docs/capabilities.md)
- PRD: [`docs/prd.md`](docs/prd.md)
- Decisions: [`docs/adr/`](docs/adr/)

**Compose products. Keep domains pure.**

A product is a statically selected set of plugins. Each plugin declares versioned
capabilities it offers and requires; Kernox validates the graph, injects typed
handles, and owns deterministic startup, rollback, and shutdown.

The graph is the control plane, not the request path. After boot, domain code
calls an ordinary `Arc<dyn Trait>` directly—no graph traversal, serialization,
event bus, or service-locator lookup per call. The absolute point-estimate delta
against direct composition is 0.14% on the recorded baseline, with overlapping
confidence intervals.

```text
product = Host + selected Plugins + explicit Bindings

build/start: descriptors -> capability DAG -> validate -> initialize -> ready
hot path:    domain code  -> direct typed handle -> provider
shutdown:    quiesce -> stop -> dispose in reverse dependency order
```

## What Kernox gives you

- deterministic provider selection, explicit ambiguity resolution, conflicts,
  semantic versions, optional/multi requirements, cycles, non-fatal graph
  diagnostics, and hard graph bounds;
- typed atomic provisioning with declared-only access and no global resolver;
- rollback that keeps the primary failure and every cleanup failure;
- reverse-order idempotent shutdown and privacy-safe lifecycle observations;
- supervised Tokio tasks with cancellation, panic fail-closed reporting,
  bounded drain, leak naming, and forced abort after the declared budget;
- provider-neutral warm serverless apps with a fresh scope per invocation;
- `cargo kernox` graph validation/rendering, a deterministic testkit, and a
  North Star conformance oracle for verified three-plugin applications; and
- dual licensing, locked verification, advisory/license/source policy, fuzzing,
  benchmarks, MSRV checks, cross-platform CI, and trusted-publishing automation.

Kernox deliberately does not provide HTTP, storage, identity, AI, billing, an
ORM, a generic event bus, or business policy. Those are plugins or external
services. Native plugins are trusted in-process Rust code, not a sandbox.

## Crates

| Package | Role |
| --- | --- |
| `kernox` | Facade with opt-in `tokio` and `serverless` features |
| `kernox-core` | Pure deterministic graph and schema contracts |
| `kernox-runtime` | Typed provisions, lifecycle, scopes, observations |
| `kernox-host-tokio` | Named supervised task capability |
| `kernox-host-serverless` | Warm app and fresh invocation host |
| `kernox-testkit` | Duration-free recorder and lifecycle failure probes |
| `cargo-kernox` | Bounded JSON validation and JSON/DOT graph inspection |

The examples cover distinct composition shapes:

- [order-app](examples/order-app) reuses one domain graph under long-lived and
  warm serverless hosts;
- [checkout-app](examples/checkout-app) swaps two payment adapters through an
  explicit binding without changing the checkout domain; and
- [worker-app](examples/worker-app) delegates a named background task to the
  supervised Tokio host and drains it on shutdown.

## Try the source candidate

```bash
cargo run -p kernox-example-order-app --bin long_lived
cargo run -p kernox-example-order-app --bin serverless
cargo run -p kernox-example-checkout-app --bin checkout -- wallet
cargo run -p kernox-example-worker-app --bin worker
cargo run --locked --manifest-path fixtures/clean-consumer/Cargo.toml
cargo run -p cargo-kernox -- kernox check fixtures/compositions/valid.json
cargo run -p cargo-kernox -- kernox check fixtures/compositions/verified.json --verified
cargo run -p cargo-kernox -- kernox graph fixtures/compositions/valid.json --format dot
```

The repository commit build is:

```bash
cargo run --locked -p xtask -- verify
```

It runs formatting, all-target checks, Clippy, tests, rustdoc, the runtime-free
core boundary, both product paths, dependency policy, RustSec audit, and the
independently packageable core artifact. Fuzz, mutation, and benchmark
distributions have separate extended lanes.

## Design and operating contract

- [Product vision](docs/vision.md)
- [Capability architecture](docs/capabilities.md)
- [Product identity and North Star](PROJECT.md)
- [Product requirements](docs/prd.md)
- [Critical path and redesign triggers](docs/critical-path.md)
- [Static graph architecture ADR](docs/adr/20260815T185400Z-static-capability-graph.md)
- [Runtime semantics](docs/specs/20260815T185400Z-runtime-contract.md)
- [Production acceptance matrix](docs/specs/20260815T185400Z-acceptance.md)
- [Standalone cardinality adopter](docs/specs/20260816-standalone-cardinality-adopter.md)
- [Plugin authoring](docs/plugin-authoring.md)
- [Compatibility](docs/compatibility.md)
- [Performance evidence](docs/performance.md)
- [Threat model](docs/security/threat-model.md)
- [Security reporting](SECURITY.md)

## Release state

Kernox is currently a pre-1.0 development engine. The workspace uses the
`0.1.x` package train; no stable 1.0 publication is permitted while the public
API and lifecycle contracts are still evolving.

Source correctness, pull-request CI, merge state, crates.io packages, and real
product adoption are separate facts. Consult GitHub Actions/Releases and the
crates.io package pages for those current states; this README does not turn a
local or merged candidate into a published release.

## License

Kernox is licensed under Apache-2.0 OR MIT, at your option. See
[`LICENSE-APACHE`](LICENSE-APACHE) and [`LICENSE-MIT`](LICENSE-MIT).
