# Kernox — local agent notes

The installed Sylphx Agent Runtime Constitution and `SylphxAI/skills` own the
static engineering and delivery standards. This file adds only repository-local
facts; it must not fork those authorities.

Repository truth lives in `PROJECT.md`, `docs/`, public Rust contracts, tests,
benchmarks, and Git history. This repository is independent of existing Sylphx
products and must not import or mutate them.

## Commands

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo doc --workspace --all-features --no-deps`
- `cargo run -p xtask -- verify` once the verification entrypoint lands

## Boundary hazards

- Never commit credentials, `.env` files, tokens, or private threat details.
- `kernox-core` must remain independent of Tokio, transports, providers, and
  product-domain policy.
- Native static plugins share the host process and are trusted code; never
  describe that boundary as sandboxed.
- Keep source, CI, package publication, and runtime adoption claims separate.
