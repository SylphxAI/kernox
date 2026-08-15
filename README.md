# Kernox

**Compose products. Keep domains pure.**

Kernox is a high-performance, graph-backed application kernel for Rust. It
assembles statically selected plugins, validates their capability graph, injects
typed dependencies, and orchestrates deterministic lifecycle transitions. Once
the graph is resolved, application calls use direct Rust handles rather than
traversing the graph or dispatching through an event bus.

Kernox is being developed as an independent open-source product. It is not
wired into any existing Sylphx product, and it does not provide HTTP, storage,
identity, AI, billing, or other business capabilities.

## Product contract

- [Project identity](PROJECT.md)
- [Product requirements](docs/prd.md)
- [Critical path](docs/critical-path.md)
- [Architecture decision](docs/adr/20260815T185400Z-static-capability-graph.md)
- [Runtime contract](docs/specs/20260815T185400Z-runtime-contract.md)
- [Acceptance matrix](docs/specs/20260815T185400Z-acceptance.md)
- [Security](SECURITY.md)

## Status

The repository and product contracts are established. Source, CI, package,
release, and adoption states are reported independently; no production-readiness
claim is implied by this document.

## License

Kernox is intended to be available under either Apache-2.0 or MIT, at your
option. The complete license files are part of the release gate.
