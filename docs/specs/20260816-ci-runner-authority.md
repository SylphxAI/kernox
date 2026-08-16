# CI runner authority

## Decision

Every repository-owned workflow job must run on an approved Sylphx Platform
self-hosted profile. The contract read from Platform `origin/main` at
`82216476383f0569f34de3d8acd557894fc7c8ba` permits the following profiles for
Kernox:

- normal commit, extended, and release work: `sylphx-linux-standard`;
- a real macOS portability lane: `[self-hosted, sylphx, macos, standard]`.

GitHub-hosted labels (`ubuntu-latest`, `macos-latest`, and `windows-latest`),
generic self-hosted selectors, invented labels, and expression-driven
`runs-on` values are not delivery evidence and are forbidden. Each job chooses
one static profile.

## Cross-platform requirement

The public workspace portability requirement remains active. Kernox retains a
macOS lane because the Platform contract has an approved macOS profile. The
Windows lane is currently an explicit acceptance residual: Platform has not
published an approved static Windows profile, so this repository does not use a
hosted runner, invent a label, or claim Windows evidence. The requirement can
close only after Platform publishes that profile and the lane runs on it.

Removing the former hosted Windows job is therefore a policy hard cut, not a
portability waiver. Linux and macOS results must never be reported as Windows
coverage.

## Evidence states

Landed source `7ef8db3b475eea6716516cfcbdadd617e265896f` is preserved. Earlier
green checks from PR `#10` (`31934513495`, `31934603072`) ran on GitHub-hosted
machines and remain source/test evidence only; they do not prove compliance
with this runner authority. A compliant CI claim requires the new workflows to
execute on the static Sylphx labels above.

The `xtask verify` entrypoint checks workflow runner declarations before the
product verification path, so a future hosted or dynamic selector fails the
repository commit build locally and in CI.
