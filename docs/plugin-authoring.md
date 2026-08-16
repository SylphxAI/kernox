# Plugin authoring

Kernox plugins are coarse composition and ownership units, not wrappers around
every function. A domain, application use case, adapter, or host integration is
a good plugin when it has a stable capability contract and resources whose
lifecycle needs an owner.

## 1. Define a domain port and marker

The trait is the callable contract. A zero-sized marker binds it to a stable
identity and exact semantic version.

```rust
use kernox::Capability;

pub trait Clock: Send + Sync {
    fn now_millis(&self) -> u128;
}

pub struct ClockCapability;

impl Capability for ClockCapability {
    type Interface = dyn Clock;
    const ID: &'static str = "dev.example.clock";
    const VERSION: &'static str = "1.0.0";
}
```

Changing trait behavior incompatibly requires a capability major-version
change. Display names and crate names are not identities.

## 2. Declare the plugin graph contract

Construct one immutable `PluginDescriptor` with offers, requirements,
conflicts, version, and source attribution. A single-provider ambiguity fails
unless the product supplies an explicit `Binding`. Host needs
are ordinary capabilities: a background worker that needs supervised Tokio
tasks declares `dev.kernox.host.tokio.tasks`; without the host plugin, graph
resolution fails before readiness.

Kernox snapshots `descriptor()` once before resolution. Do not mutate or derive
it from live configuration; configuration values belong in the plugin instance
behind that stable contract.

## 3. Initialize transactionally

During `initialize`, use `InitializationContext::require`, `optional`, or `all`
only for capabilities declared by this descriptor. Keep the returned direct
handles in the plugin or in the implementation it publishes.

Return a `ProvisionSet` containing every declared offer exactly once. Kernox
validates identity, version, and Rust marker type before committing the whole
set. On error or mismatch, none of that plugin's provisions become visible and
`dispose` is called for partial local cleanup.

## 4. Own the full lifecycle

- `initialize`: construct resources and staged provisions;
- `start`: activate work only after all provisions are committed;
- `quiesce`: close admission and signal cancellation;
- `stop`: drain or stop owned work;
- `dispose`: release resources and tolerate partial initialization.

Expected failures return `PluginError` with a stable static tag. Do not panic
for caller-controlled input. If a hook still unwinds, Kernox reports
`plugin.hook-panicked` and continues transactional rollback or later cleanup
hooks; this is not memory isolation or a sandbox. A native plugin runs with
host-process privilege, and `panic=abort` remains a process failure.

## 5. Keep architecture direction inward

Domain traits and functional cores must not depend on Kernox. The plugin is the
imperative composition shell that implements or connects those ports. This
supports DDD bounded contexts, Clean Architecture dependency direction,
Hexagonal ports/adapters, and Functional Core/Imperative Shell without making
those methods part of the kernel.

Removing a plugin means removing its registration and repairing any now-missing
requirements. It does not imply runtime unloading of native machine code.

## 6. Verify an application

The North Star conformance oracle consumes a resolved application and exercises
its real initialization, startup, and shutdown path:

```rust,no_run
use kernox_testkit::verify_application;

# async fn example(app: kernox_runtime::ResolvedApp) {
let report = verify_application(app).await?;
assert_eq!(report.plugin_count, 3);
# Ok::<(), kernox_testkit::ConformanceError>(())
# }
```

Conformance requires at least three plugins, complete source attribution, and
unique source package names. The graph shape can be checked without startup:

```bash
cargo run -p cargo-kernox -- kernox check fixtures/compositions/verified.json --verified
```

A passing check or `verify_application` proves composition and, for the
testkit, lifecycle behavior; it does not by itself prove independent legal
ownership, registry publication, or deployment.

See [`examples/order-app/src/lib.rs`](../examples/order-app/src/lib.rs) for a
three-plugin implementation used unchanged by two Hosts.
