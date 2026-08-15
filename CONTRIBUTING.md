# Contributing to Kernox

Kernox accepts focused issues and pull requests that preserve its product
contract, architecture boundary, and public compatibility policy.

Before opening a change:

1. Read `PROJECT.md`, `docs/prd.md`, the runtime contract, and the relevant ADR.
2. Add an executable regression or acceptance oracle for changed behavior.
3. Run `cargo run -p xtask -- verify` with the pinned Rust toolchain.
4. Explain public API, performance, security, and compatibility effects in the
   pull request. Include benchmark evidence when the hot path changes.

Expected failures return typed errors; do not add panics for caller-controlled
input. Project-owned Rust forbids unsafe code. Native plugins remain trusted
in-process code and must never be described as sandboxed.

By submitting a contribution, you agree that it is licensed under the same
Apache-2.0 OR MIT terms as Kernox.
