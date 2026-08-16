# Performance evidence

Performance is an acceptance contract, not a permanent claim from one machine.
The scheduled benchmark workflow reruns the same source benchmark and retains
its raw output. Changes to graph resolution, typed provision lookup, or handle
shape must be re-measured.

## Baseline — 2026-08-15

Command:

```text
cargo bench --locked -p kernox --bench kernel -- \
  --sample-size 20 --measurement-time 1 --warm-up-time 1
```

Environment: Linux 6.18.18 x86_64, AMD EPYC 9454, rustc 1.97.1, optimized
Cargo bench profile. Reported values below are Criterion confidence intervals
with the point estimate in the middle.

| Path | Time |
| --- | ---: |
| 10 plugins, sparse graph | 9.7476–10.138 µs (9.9167 µs) |
| 10 plugins, dense DAG | 27.445–28.029 µs (27.730 µs) |
| 100 plugins, sparse graph | 124.45–125.58 µs (124.98 µs) |
| 100 plugins, dense DAG | 3.6083–3.6404 ms (3.6245 ms) |
| 1,000 plugins, sparse graph | 1.5565–1.6656 ms (1.5945 ms) |
| 1,000 plugins, dense DAG (499,500 edges) | 752.26–764.56 ms (757.69 ms) |
| Direct `Arc<dyn Trait>` call | 1.3207–1.3362 ns (1.3260 ns) |
| Kernox-extracted `Arc<dyn Trait>` call | 1.3223–1.3261 ns (1.3241 ns) |

The absolute point-estimate delta for the steady-state call is approximately
0.14%, and the confidence intervals overlap. This passes the declared 2% budget
on this environment. The Kernox point estimate happened to be lower, which is
treated as measurement noise rather than an acceleration claim. Both measured
hot paths are the same direct dynamic trait call after boot; Kernox does not
perform a graph lookup, registry lookup, serialization, or event dispatch per
call.

Dense graph construction is intentionally a control-plane stress case. The
499,500-edge case includes stable graph-diagnostic construction and remains
below one second on this baseline. It is bounded by absolute
node/declaration/edge ceilings and is not part of normal application request
processing.

## Development matrix — 2026-08-16

The benchmark was expanded on the pre-1.0 development candidate with lifecycle
and warm-scope paths:

```text
cargo bench --locked -p kernox --bench kernel -- \
  --sample-size 10 --measurement-time 0.25 --warm-up-time 0.25
```

Environment: Linux 6.18.18 x86_64, AMD EPYC 9454, rustc 1.97.1. This short
run is a diagnostic snapshot, not a replacement for the scheduled distribution
baseline.

| Path | Time |
| --- | ---: |
| 1-plugin boot + reverse shutdown | 1.764–1.828 µs |
| 3-plugin boot + reverse shutdown | 4.038–4.249 µs |
| Warm invocation scope open + close | 58.83–63.91 ns |
| 256-requirement indexed lookup | 82.90–83.32 ns |

A repeated 50-sample steady-state run measured direct dispatch at
`1.3334–1.3446 ns` and a Kernox-extracted direct handle at `1.3325–1.3388 ns`
in the same process. The point estimates were within 0.3%; an earlier run on
the same machine showed a roughly 2% spread, which is why a single Criterion
comparison is not treated as a product regression or an acceleration claim.

## What this does not prove

Kernox is not yet “optimized to the limit.” The evidence proves that the
post-composition call shape is direct typed-handle dispatch and that the tested
control-plane paths are bounded on one machine. It does not yet characterize
allocator profiles, cold-start distributions, high-concurrency contention,
provider I/O, end-to-end application latency, cache behavior across CPUs, or
tail latency under load. Those are explicit follow-up measurements before any
stable 1.0 decision; optimization work should follow a measured bottleneck
instead of adding speculative machinery to the core.
