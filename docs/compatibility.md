# Compatibility policy

Kernox is intentionally pre-1.0 during the long engine-development phase. The
workspace currently uses the `0.1.x` package train, and public APIs, lifecycle
semantics, and descriptor/report contracts may still change between minor
development releases. No stable 1.0 publication is allowed until the engine,
examples, performance evidence, and adoption path have been reviewed as
mature.

Once a stable 1.x release is explicitly admitted, it will follow Semantic
Versioning and will not remove or incompatibly change public items without a
major release.
Deprecations name a replacement and remain for at least one minor release
before the next permitted major removal.

Capability identities and versions are application contracts. Compatible
providers may advance within the consumer's `VersionReq`; incompatible trait or
semantic behavior requires a capability major-version change. Explicit product
bindings select among compatible providers and are never inferred from build
order.

Serialized composition and graph reports carry their own schema version.
Readers reject unsupported versions instead of guessing. Stable error tags are
machine contracts within a major line; display prose is not.

Static Rust source compatibility is not an ABI promise. A future process or
WebAssembly Component plugin boundary requires a separate accepted ABI/WIT,
capability-grant, resource, and migration contract. Native dynamic libraries
are not a supported extension mechanism.

CI compares public library APIs with published predecessors when they exist.
Before the first registry release there is no predecessor, so source review and
consumer examples are the available compatibility evidence.
