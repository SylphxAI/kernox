# Clean consumer fixture

This is a standalone Cargo package with its own workspace boundary. It uses
only the public `kernox` facade and `kernox-testkit` path dependencies, then
runs the real three-plugin conformance path. It is deliberately outside the
root workspace so `cargo run --locked --manifest-path
fixtures/clean-consumer/Cargo.toml` exercises a consumer-shaped build rather
than another workspace member.

Passing this fixture proves source-level consumer integration. It does not
prove registry publication, independent legal ownership, deployment, or live
adoption; those remain separate North Star facts.
