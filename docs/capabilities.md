# Experimental engine capability graph

This file is the implemented capability DAG of the pre-1.0 Kernox engine
(workspace package train `0.1.x` on this revision). It is not a product
requirements document, a release plan, or an adoption claim.

| Home | Owns |
| --- | --- |
| [`docs/prd.md`](prd.md) | Product promise, KR-* inventory, non-goals, release terminal |
| [`docs/specs/20260815T185400Z-runtime-contract.md`](specs/20260815T185400Z-runtime-contract.md) | Identifier, resolution, provision, lifecycle, scope, host semantics |
| [`docs/specs/20260815T185400Z-acceptance.md`](specs/20260815T185400Z-acceptance.md) | Release-claim to oracle matrix |
| [`docs/adr/20260815T185400Z-static-capability-graph.md`](adr/20260815T185400Z-static-capability-graph.md) | Why the graph is the kernel authority |
| This file | What this experimental engine actually implements, the prerequisite edges, and the command that would fail if a node were false |

A green oracle is local source evidence on the checked-out revision. It is not
crates.io publication, CI-on-main, a 1.0 API freeze, or live adoption.

## How to read the DAG

- A **node** is one implemented engine capability with a stable `KNX-*` id.
- A directed edge `A -> B` means B is not independently true unless A holds.
- Nodes exist only when this revision has source and an executable oracle.
- An **oracle** is a command plus a falsifier. If the capability is missing or
  inverted, that command must fail for the stated reason.
- Stable error tags are the machine contract. Display prose is not.
- Native plugins share the host process. No node here is a sandbox.

Schema constants on this revision:

| Contract | Value |
| --- | --- |
| `COMPOSITION_SCHEMA_VERSION` | `1` |
| `GRAPH_REPORT_SCHEMA_VERSION` | `1` |
| Default `GraphLimits` | 4,096 plugins / 512 declarations per plugin / 131,072 edges |
| Absolute ceilings | 65,536 / 4,096 / 1,048,576 |
| `MINIMUM_VERIFIED_PLUGINS` | `3` |
| `cargo kernox` input bound | 16 MiB |
| Fuzz target input bound | 1 MiB |

## Package DAG

Control-plane crates do not depend on hosts. Hosts and the facade depend on
runtime. Core has no Tokio, no default Serde, and no I/O.

```text
kernox-core
  ├─ cargo-kernox
  └─ kernox-runtime
       ├─ kernox-host-tokio
       ├─ kernox-host-serverless
       ├─ kernox-testkit
       └─ kernox
            ├─ examples/order-app
            ├─ examples/checkout-app
            ├─ examples/worker-app
            └─ fixtures/clean-consumer   (separate manifest)
```

Release package order from `xtask` is
`kernox-core -> kernox-runtime -> kernox-host-serverless -> kernox-host-tokio ->
kernox-testkit -> kernox -> cargo-kernox`.

## Engine capability DAG

Edges point from prerequisite to dependent. `KNX-GRAPH` is the kernel
authority: invalid compositions fail before any plugin hook runs.

```text
KNX-ID -> KNX-GRAPH
KNX-GRAPH -> KNX-REPORT -> KNX-INSPECT
KNX-GRAPH -> KNX-ATTRIB -> KNX-CONFORM
KNX-GRAPH -> KNX-CORE-INDEP
KNX-GRAPH -> KNX-FUZZ
KNX-GRAPH -> KNX-PROVISION -> KNX-HOTPATH
KNX-PROVISION -> KNX-LIFECYCLE
KNX-LIFECYCLE -> KNX-OBS
KNX-LIFECYCLE -> KNX-ADMISSION
KNX-LIFECYCLE -> KNX-TESTKIT -> KNX-CONFORM
KNX-LIFECYCLE -> KNX-SCOPE
KNX-SCOPE -> KNX-HOST-TOKIO -> KNX-EX-WORKER
KNX-SCOPE -> KNX-HOST-SERVERLESS -> KNX-EX-ORDER
KNX-CONFORM -> KNX-EX-ORDER
KNX-GRAPH -> KNX-EX-CHECKOUT
KNX-LIFECYCLE -> KNX-EX-CHECKOUT
KNX-PROVISION -> KNX-EX-CONSUMER
KNX-HOST-SERVERLESS -> KNX-EX-CONSUMER
KNX-CONFORM -> KNX-EX-CONSUMER
```

`KNX-EX-*` nodes are landed reference compositions the engine actually
resolves. They are evidence that the kernel DAG is usable, not product
features of Kernox.

## Node register

| ID | Capability | Depends on | Oracle | Falsifier |
| --- | --- | --- | --- | --- |
| `KNX-ID` | Validated plugin/capability identities, versions, offers, requirements, conflicts, source | — | `cargo test --locked -p kernox-core --all-features identifier`; `cargo test --locked -p kernox-core --all-features descriptor_` | Malformed, duplicate, or self-conflicting constructors succeed |
| `KNX-GRAPH` | Pure, insertion-order-independent provider resolution with bindings, cardinalities, cycles, conflicts, and hard limits | `KNX-ID` | `cargo test --locked -p kernox-core --all-features graph` | Invalid composition becomes ready, or a valid graph changes under descriptor permutation |
| `KNX-REPORT` | Schema-versioned `GraphReport`; teardown is the exact reverse of startup; unknown majors and inconsistent projections fail closed | `KNX-GRAPH` | `cargo test --locked -p kernox-core --all-features graph_report` | An unsupported report major is accepted, or teardown is not the reverse of startup |
| `KNX-ATTRIB` | Graph-only verified-application attribution: at least 3 plugins, package+repository on every plugin, unique source packages | `KNX-GRAPH` | `cargo test --locked -p kernox-core --all-features attribution::`; `cargo test --locked -p cargo-kernox --all-features verified_check_requires_three_attributed_plugins` | A two-plugin or unattributed graph passes `--verified` |
| `KNX-INSPECT` | `cargo kernox check` / `graph` over bounded JSON; DOT/JSON render of the same `GraphReport` | `KNX-REPORT`, `KNX-ATTRIB` | `cargo run --locked -p cargo-kernox -- kernox check fixtures/compositions/valid.json`; `cargo run --locked -p cargo-kernox -- kernox check fixtures/compositions/verified.json --verified` | Valid fixture rejected, or `valid.json` accepted as `--verified` |
| `KNX-PROVISION` | Typed `Capability` markers, atomic `ProvisionSet` commit, declared-only `require` / `optional` / `all` | `KNX-GRAPH` | `cargo test --locked -p kernox-runtime --all-features --test lifecycle undeclared`; `cargo test --locked -p kernox-runtime --all-features --test lifecycle dependency_access_mode_must_match_declared_cardinality` | Undeclared, wrong-cardinality, or type-mismatched access returns a handle |
| `KNX-LIFECYCLE` | Deterministic startup order; transactional initialize/start; reverse rollback; hook-unwind isolation; reverse idempotent shutdown | `KNX-PROVISION` | `cargo test --locked -p kernox-runtime --all-features --test lifecycle` | Partial provisions become visible, cleanup stops after a hook panic, or a second `shutdown` re-runs hooks |
| `KNX-ADMISSION` | Root `capability_from` closes when the application scope enters `Closing` | `KNX-LIFECYCLE` | `cargo test --locked -p kernox-runtime --all-features root_capability_access_closes` | `RunningApp::capability_from` succeeds after `begin_close` / shutdown admission |
| `KNX-SCOPE` | Host-neutral scope identity, parent, closure, and invocation-lifetime borrow | `KNX-LIFECYCLE` | `cargo test --locked -p kernox-runtime --all-features scope` | Two concurrent invocations share a scope id, a child is created after parent close, or closed children leak on a long-lived parent |
| `KNX-OBS` | Privacy-safe lifecycle observations; sink unwind cannot abort remaining hooks | `KNX-LIFECYCLE` | `cargo test --locked -p kernox-runtime --all-features observation_sink_unwind_does_not_abort_lifecycle` | A panicking sink aborts rollback or shutdown |
| `KNX-TESTKIT` | Duration-free recorder and injected hook failures | `KNX-LIFECYCLE` | `cargo test --locked -p kernox-testkit --all-features --test probe` | Injected `start`/`initialize` failure is not the primary tag, or reverse dispose is skipped |
| `KNX-CONFORM` | `verify_application` consumes a `ResolvedApp`, checks attribution, boots, and requires a clean shutdown | `KNX-ATTRIB`, `KNX-LIFECYCLE`, `KNX-TESTKIT` | `cargo test --locked -p kernox-testkit --all-features --test conformance` | Fewer than three plugins, missing source, or a dirty shutdown still returns `ConformanceReport` |
| `KNX-HOST-TOKIO` | Official plugin `dev.kernox.host.tokio` offering `dev.kernox.host.tokio.tasks` with named admission, cancel, drain, force-abort, panic fail-closed | `KNX-SCOPE`, `KNX-PROVISION` | `cargo test --locked -p kernox-host-tokio --all-features --test supervision` | A cancelled cooperative task is not drained, or a stubborn task is not named and force-aborted before stop returns |
| `KNX-HOST-SERVERLESS` | Provider-neutral warm `ServerlessHost`: reused App, fresh invocation scope, capacity and closed-admission tags | `KNX-SCOPE` | `cargo test --locked -p kernox-host-serverless --all-features --test invocations` | Concurrent calls share request state, or post-shutdown `begin_invocation` is accepted |
| `KNX-HOTPATH` | After readiness, calls use the extracted `Arc<dyn Trait>`; no graph, registry, or event hop | `KNX-PROVISION` | `cargo test --locked -p kernox-runtime --all-features boots_with_direct_typed_handle` and `cargo run --release --locked --manifest-path fixtures/clean-consumer/Cargo.toml -- --workload` | A post-boot call requires graph lookup, or the 2,048-call workload exceeds p99 5 ms / max 100 ms |
| `KNX-CORE-INDEP` | `kernox-core` default features compile without Tokio/Serde and do not depend on them | `KNX-GRAPH` | `cargo check --locked -p kernox-core --no-default-features` plus `xtask` `enforce_core_dependency_boundary` via `cargo run --locked -p xtask -- verify` | Default `kernox-core` edges to `tokio`, `tokio-util`, or a host crate |
| `KNX-FUZZ` | Untrusted composition JSON is bounded and fail-closed | `KNX-GRAPH` | `cargo fuzz run graph_json` in `fuzz/` (extended lane); local bound oracle is `rejects_oversized_input_before_json_parsing` | Oversized CLI input is parsed, or the fuzz target is absent |
| `KNX-EX-ORDER` | Host-neutral three-plugin order graph, reused by long-lived and serverless binaries | `KNX-CONFORM`, `KNX-HOST-SERVERLESS` | `cargo test --locked -p kernox-example-order-app --all-features` and `cargo run --locked -p kernox-example-order-app --bin long_lived` / `--bin serverless` | `compose()` fails conformance or a host-specific domain change is required |
| `KNX-EX-CHECKOUT` | Four-plugin checkout graph; explicit binding selects card xor wallet | `KNX-GRAPH`, `KNX-LIFECYCLE` | `cargo test --locked -p kernox-example-checkout-app --all-features` | Both providers are selected without a binding, or the domain receipt ignores the bound provider |
| `KNX-EX-WORKER` | Heartbeat worker requires the official Tokio task capability | `KNX-HOST-TOKIO` | `cargo test --locked -p kernox-example-worker-app --all-features` | Shutdown leaves the heartbeat admitted, or `ticks()` stays zero |
| `KNX-EX-CONSUMER` | Out-of-workspace typed consumer: direct, warm, workload, and `all`/`optional` fan-out | `KNX-PROVISION`, `KNX-HOST-SERVERLESS`, `KNX-CONFORM` | `cargo run --locked --manifest-path fixtures/clean-consumer/Cargo.toml` and the `--serverless`, `--workload`, `--fanout` entrypoints | Fan-out order is not provider-identity order, optional metrics is not `None`, or a warm invocation leaks |

`cargo run --locked -p xtask -- verify` is the conjunction of the local
oracles above except the extended fuzz and Criterion distribution lanes.

## Resolution contract encoded by `KNX-GRAPH`

`GraphBuilder::resolve` is pure. It snapshots descriptors, then fail-closes
before readiness on every case below. Tags are from `ResolveError::tag`.

| Condition | Tag |
| --- | --- |
| Unsupported composition schema major | `graph.unsupported-schema-version` |
| Unsupported / inconsistent report | `graph.unsupported-report-schema`, `graph.inconsistent-report-lifecycle`, `graph.duplicate-report-plugin`, `graph.unknown-report-plugin` |
| Configured limit above absolute ceiling | `graph.configured-limit-exceeded` |
| Plugin / declaration / edge ceiling | `graph.plugin-limit`, `graph.capability-limit`, `graph.edge-limit` |
| Duplicate plugin identity | `graph.duplicate-plugin` |
| Declared plugin conflict present | `graph.plugin-conflict` |
| Plugin offers and requires the same capability | `graph.self-dependency` |
| Required provider missing | `graph.missing-provider` |
| Candidates exist but versions miss `VersionReq` | `graph.incompatible-provider` |
| Exactly-one / zero-or-one has multiple compatible candidates and no binding | `graph.ambiguous-provider` |
| Binding names unknown consumer/provider | `graph.unknown-binding-consumer`, `graph.unknown-binding-provider` |
| Binding does not match a bindable requirement | `graph.unused-binding` |
| Bound plugin does not offer the capability | `graph.binding-provider-does-not-offer` |
| Bound offer misses `VersionReq` | `graph.incompatible-binding` |
| Duplicate binding key | `graph.duplicate-binding` |
| Binding used on `ZeroOrMore` / `OneOrMore` | `graph.binding-for-multiple` |
| Selected provider-to-consumer edges contain a cycle | `graph.dependency-cycle` |

Selection rules this revision actually implements:

- `ExactlyOne`: one compatible provider, or a binding; else fail.
- `ZeroOrOne`: zero or one compatible provider; absence is
  `graph.optional-provider-missing`, not a hard error.
- `OneOrMore` / `ZeroOrMore`: every compatible provider, sorted by plugin
  identity. Bindings are rejected.
- Startup order is Kahn with a `BTreeSet` ready queue (lowest remaining
  identity first). Teardown is that sequence reversed.
- Unselected offers, including intentional root exports, emit
  `graph.unselected-offer`.

Oracle for the tag table:
`cargo test --locked -p kernox-core --all-features graph_contract_failures_have_stable_repair_tags`.
Oracle for insertion invariance:
`independent_plugin_order_is_insertion_invariant` and
`dependency_chain_order_is_insertion_invariant`.

## Provision and lifecycle oracles

Supported access is only through `InitializationContext` methods that match
the declared cardinality. The context cannot escape as `'static`
(`crates/kernox-runtime/src/capability.rs` compile-fail). Invocation scope
views cannot escape (`InvocationScope` compile-fail in
`crates/kernox-runtime/src/app.rs`).

| Event | Tag / behavior | Oracle |
| --- | --- | --- |
| Staged undeclared offer | `provision.undeclared` | `undeclared_and_version_mismatched_provisions_fail_before_readiness` |
| Missing declared offer | `provision.missing` | `missing_provision_is_not_committed_and_current_plugin_is_disposed` |
| Marker vs descriptor version | `provision.version-mismatch` | `undeclared_and_version_mismatched_provisions_fail_before_readiness` |
| Duplicate stage | `provision.duplicate` | `duplicate_staged_provision_is_rejected_locally` |
| Undeclared consume | `access.undeclared` | `undeclared_capability_access_fails_closed` |
| Wrong `require`/`optional`/`all` | `access.cardinality-mismatch` | `dependency_access_mode_must_match_declared_cardinality` |
| Same identity, different marker | `access.type-mismatch` | `same_identity_with_a_different_marker_type_fails_closed` |
| Root access after shutdown admission | `access.application-unavailable` | `root_capability_access_closes_when_shutdown_begins` and `boots_with_direct_typed_handle_and_shuts_down_in_reverse_order_once` |
| Hook or builder unwind | `plugin.hook-panicked`; reverse dispose continues | `initialize_hook_unwind_rolls_back_already_initialized_plugins`, `start_hook_unwind_runs_reverse_cleanup`, `cleanup_hook_unwind_continues_later_hooks` |

Handles acquired before admission closure stay direct. They are not revoked.
That residual is documented, not a hole in `KNX-ADMISSION`.

## Landed composition DAGs

These are the graphs this experimental engine resolves in-tree. Plugin
identities and capability identities are the ones the examples declare.

### `KNX-EX-ORDER` — `examples/order-app`

```text
dev.kernox.examples.order-store
  --[dev.kernox.examples.orders.store ExactlyOne ^1.0]-->
    dev.kernox.examples.order-service
dev.kernox.examples.system-clock
  --[dev.kernox.examples.clock ExactlyOne ^1.0]-->
    dev.kernox.examples.order-service
dev.kernox.examples.order-service
  provides dev.kernox.examples.orders.service   (root export)
```

Ready-set lexical startup on this identity set is
`order-store -> system-clock -> order-service`. The same `compose()` feeds
`long_lived`, `serverless`, and
`kernox_testkit::verify_application`. Source packages
`kernox-example-order-store`, `kernox-example-system-clock`,
`kernox-example-order-service` must stay unique.

### `KNX-EX-CHECKOUT` — `examples/checkout-app`

Both payment plugins are always installed. One `Binding` selects the
`ExactlyOne` payment provider. Removing the binding is
`graph.ambiguous-provider`.

```text
dev.kernox.examples.checkout.inventory
  --[dev.kernox.examples.checkout.inventory ExactlyOne ^1.0]-->
    dev.kernox.examples.checkout.service
{dev.kernox.examples.checkout.card-payment
 | dev.kernox.examples.checkout.wallet-payment}
  --[dev.kernox.examples.checkout.payment ExactlyOne ^1.0 + Binding]-->
    dev.kernox.examples.checkout.service
```

The unbound payment plugin remains in the graph with
`graph.unselected-offer`. Domain code sees only `dyn PaymentGateway`.

### `KNX-EX-WORKER` — `examples/worker-app`

```text
dev.kernox.host.tokio
  --[dev.kernox.host.tokio.tasks ExactlyOne ^1.0]-->
    dev.kernox.examples.worker.heartbeat
dev.kernox.examples.worker.heartbeat
  provides dev.kernox.examples.worker.metrics
```

Without `TokioTaskPlugin`, resolution fails with `graph.missing-provider`
before the worker starts. Shutdown must drain the named `heartbeat` task.

### `KNX-EX-CONSUMER` — `fixtures/clean-consumer`

Default graph:

```text
dev.kernox.clean-consumer.clock
  --[dev.kernox.clean-consumer.clock ExactlyOne]-->
    dev.kernox.clean-consumer.greeting
      --[dev.kernox.clean-consumer.greeting ExactlyOne]-->
        dev.kernox.clean-consumer.application
```

Fan-out graph (`--fanout`):

```text
dev.kernox.clean-consumer.notifier-email
dev.kernox.clean-consumer.notifier-webhook
        \                     /
         --[notifier OneOrMore / all]-->
              dev.kernox.clean-consumer.fanout
dev.kernox.clean-consumer.metrics
  ZeroOrOne / optional -> None
  diagnostic graph.optional-provider-missing
```

Dispatch order is provider identity order (`notifier-email` then
`notifier-webhook`). The fixture is a separate Cargo package and does not
share the workspace `[workspace.dependencies]` graph.

### Offline fixtures

| File | Graph | Oracle |
| --- | --- | --- |
| `fixtures/compositions/valid.json` | `dev.example.clock` -> `dev.example.orders` (unattributed, two plugins) | `cargo kernox check` succeeds; `--verified` must fail `conformance.too-few-plugins` |
| `fixtures/compositions/verified.json` | `clock` + `store` -> `orders`, three unique sources | `cargo kernox check --verified` succeeds |

## Host tags

| Host | Tag | Meaning |
| --- | --- | --- |
| Tokio | `tokio-task.invalid-name` | Empty, oversized, or control-character task name |
| Tokio | `tokio-task.closed` | Spawn after quiesce |
| Tokio | `tokio-task.capacity` | `max_tasks` exhausted |
| Tokio | `tokio-task.no-runtime` | No current Tokio handle |
| Tokio | `tokio-task.drain-timeout` | Stubborn task exceeded drain budget and was force-aborted |
| Tokio | `tokio-task.panicked` | Supervised task panicked; payload is not retained |
| Serverless | `serverless.closed` | Invoke after host shutdown |
| Serverless | `serverless.capacity` | `max_concurrent_invocations` exhausted |

## Absent from this revision

These are not nodes. Do not treat a missing row as implemented.

- Host capability negotiation before readiness (named in the runtime
  contract; no selecting implementation on this revision).
- Out-of-process or WebAssembly Component plugins, native `dlopen`, or any
  isolation claim for trusted in-process plugins.
- A generic event bus, service locator, or graph walk on the request path.
- Business capabilities (HTTP, identity, storage, billing, AI, queues).
- A 1.0 compatibility freeze, registry publication, or live adopter.

If a future change adds a node, it lands with a prerequisite edge and a
command that would fail if the node were false. If a future change removes
behavior, delete the node in the same source change as the oracle.
