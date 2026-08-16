# Clean consumer fixture

This is a standalone Cargo package with its own workspace boundary. It uses
only the public `kernox` facade and `kernox-testkit` path dependencies, then
composes three source-attributed plugins with typed capability requirements.
The application obtains a direct typed handle, executes a domain call, shuts
down, and runs the real three-plugin conformance path. It is deliberately
outside the root workspace so `cargo run --locked --manifest-path
fixtures/clean-consumer/Cargo.toml` exercises a consumer-shaped build rather
than another workspace member.

The same package also has a bounded workload mode:

```text
cargo run --release --locked --manifest-path fixtures/clean-consumer/Cargo.toml -- --workload
```

It calls the exported application capability from four threads for 512 calls
each, checks every domain result, and reports p50/p95/p99/max call latency. The
development oracle rejects p99 above 5 ms or any call above 100 ms. These are
short-run regression guardrails for this fixture, not a universal service-level
objective or a sustained-load claim.

Passing this fixture proves source-level consumer integration. It does not
prove registry publication, independent legal ownership, deployment, or live
adoption; those remain separate North Star facts.
