# Kernox

Kernox is an experimental Rust library for assembling an application from
plugins. Each plugin declares what it provides and what it needs; Kernox checks
the whole graph at startup, wires the plugins together, and starts, rolls back
and shuts them down in dependency order.

The graph is used only at startup and shutdown. After boot, your code calls an
ordinary `Arc<dyn Trait>` directly, with no graph lookup, serialization or
event bus per call. In the recorded benchmark the difference from wiring the
same code by hand is 0.14%, within measurement noise
([performance](docs/performance.md)).

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
- supervised Tokio tasks with cancellation, panic reporting, a bounded drain,
  naming of leaked tasks, and a forced abort after the declared time budget;
- provider-neutral warm serverless apps with a fresh scope per invocation;
- `cargo kernox` to validate and draw a graph, and a deterministic testkit; and
- fuzzing, benchmarks, minimum-Rust-version checks and cross-platform CI.

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
- [checkout-app](examples/checkout-app) is a CLI-without-Tokio product: it
  composes through `AppBuilder` and swaps two payment adapters through an
  explicit binding without changing the checkout domain; and
- [worker-app](examples/worker-app) delegates a named background task to the
  supervised Tokio host and drains it on shutdown.

## Try it

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

Kernox is not on crates.io yet (the `0.0.1` crate there only reserves the
name), so use it from Git. To run every check CI runs:

```bash
cargo run --locked -p xtask -- verify
```

It runs formatting, Clippy, tests, rustdoc, the dependency and license policy,
a RustSec audit and a secret scan. Fuzzing, mutation testing and benchmarks run
separately.

## Documentation

- [Product vision](docs/vision.md)
- [Capability architecture](docs/capabilities.md)
- [Product requirements](docs/prd.md)
- [Critical path and redesign triggers](docs/critical-path.md)
- [Runtime semantics](docs/specs/20260815T185400Z-runtime-contract.md)
- [Acceptance matrix](docs/specs/20260815T185400Z-acceptance.md)
- [Standalone cardinality adopter](docs/specs/20260816-standalone-cardinality-adopter.md)
- [Plugin authoring](docs/plugin-authoring.md)
- [Compatibility](docs/compatibility.md)
- [Performance evidence](docs/performance.md)
- [Threat model](docs/security/threat-model.md)
- [Security reporting](SECURITY.md)

## Status

Kernox is pre-1.0 and experimental: the API can change in any `0.x` release.
Releases are listed in [CHANGELOG.md](CHANGELOG.md).

## License

Kernox is licensed under Apache-2.0 OR MIT, at your option. See
[`LICENSE-APACHE`](LICENSE-APACHE) and [`LICENSE-MIT`](LICENSE-MIT).
