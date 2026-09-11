# Contributing to Kernox

Kernox accepts focused issues and pull requests that preserve its product
contract, architecture boundary, and public compatibility policy.

Before opening a change:

1. Read `PROJECT.md`, `docs/vision.md`, `docs/capabilities.md`, `docs/prd.md`, and the runtime contract.
2. Add an executable regression or acceptance oracle for changed behavior.
3. Run `cargo run -p xtask -- verify` with the pinned Rust toolchain. The
   entrypoint owns a pinned secret-scan oracle (`gitleaks` v8.30.1, linux
   x86_64): it uses a scanner whose bytes match the pinned release binary
   (sha256 `88f91962aa2f93ac6ab281d553b9e125f5197bbbce38f9f2437f7299c32e5509`)
   from `PATH` or `target/kernox-tools`, downloads and checksum-verifies the
   pinned tarball (sha256
   `551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb`) on
   first use, rejects repository-controlled `.gitleaks.toml` and
   `.gitleaksignore` suppression files, and fails closed on other hosts.
4. Explain public API, performance, security, and compatibility effects in the
   pull request. Include benchmark evidence when the hot path changes.

Expected failures return typed errors; do not add panics for caller-controlled
input. Project-owned Rust forbids unsafe code. Native plugins remain trusted
in-process code and must never be described as sandboxed.

By submitting a contribution, you agree that it is licensed under the same
Apache-2.0 OR MIT terms as Kernox.
