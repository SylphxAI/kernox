# CI runner authority

## Decision

Every repository-owned workflow job runs on a Sylphx-owned runner (owner
`standards/dx.md`, SylphxAI/owner #746). The GitHub Actions budget is $0.
Kernox uses:

- commit, extended, and release work: `sylphx-linux-standard`;
- mutation testing, which rebuilds the workspace per mutant:
  `sylphx-linux-xlarge`.

GitHub-hosted labels (`ubuntu-*`, `macos-*`, `windows-*`), generic
self-hosted selectors, invented labels, and expression-driven `runs-on` values
are forbidden. Each job chooses one static profile. This supersedes the
2026-09-24 reading that public repositories must use GitHub-hosted runners.

## Cross-platform requirement

The public workspace portability requirement remains active. Kernox has no
platform-specific code and ships crates, not prebuilt binaries, so there is
nothing to cross-compile for release. The former hosted `macOS portability`
job is replaced by `cross-target portability`: a Linux `cargo check` of every
published library and binary for `aarch64-apple-darwin`,
`x86_64-apple-darwin`, and `x86_64-pc-windows-msvc`.

That lane is compile evidence only. Linux results and cross-target compiles
must never be reported as macOS or Windows test coverage; on-platform test
evidence remains an explicit acceptance residual.

## Evidence states

Earlier green checks that ran on GitHub-hosted machines remain source/test
evidence only; they do not prove compliance with this runner authority. A
compliant CI claim requires the workflows to execute on the Sylphx labels
above.

The `xtask verify` entrypoint parses each workflow job's `runs-on` value before
the product verification path, so a hosted or dynamic selector fails the
repository commit build locally and in CI. A comment that mentions a hosted
label is not a runner assignment.
