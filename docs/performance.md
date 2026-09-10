# Performance evidence

Performance is an acceptance contract, not a permanent claim from one machine.
The scheduled benchmark workflow reruns the same source benchmark and retains
its raw output. Changes to graph resolution, typed provision lookup, or handle
shape must be re-measured.

## Steady-state budget gate — 2026-09-10

The steady-state comparison is enforced as a fail-closed gate, not recorded as
a single observation. `cargo run --locked -p xtask -- bench-budget
[--criterion-dir DIR]` reads the Criterion 0.8.2 machine-readable estimates from
the default directory `target/criterion`:

- `steady-state-call/direct-arc-dyn-trait/new/estimates.json`
- `steady-state-call/kernox-extracted-arc-dyn-trait/new/estimates.json`

The gate uses only `mean.point_estimate` and
`mean.confidence_interval.lower_bound` / `mean.confidence_interval.upper_bound`
(nanoseconds), computes

```text
delta = (kernox_point - direct_point) / direct_point
```

and passes when `delta <= 0.02`. Exactly 2% passes because the claim is "no
greater than 2%"; any delta above 2% fails with a non-zero exit. The two
`mean.confidence_interval` ranges are also compared as closed intervals and
reported as overlapping or not; overlap is informational and never waives the
point-estimate budget. Missing estimate files, an absent benchmark, unparsable
JSON, and missing, non-numeric, non-finite, non-positive, or out-of-order
fields fail closed with a message naming the path and field. Every run prints
`bench-budget.direct=<ns>`, `bench-budget.kernox=<ns>`,
`bench-budget.direct.interval=[lo, hi]`, `bench-budget.kernox.interval=[lo, hi]`,
`bench-budget.delta=<fraction>`, `bench-budget.overlap=true|false`, and
`bench-budget.result=pass|fail`. The console `time:` line Criterion prints is
its slope estimate; the gate deliberately reads `mean`.

The scheduled extended lane runs `cargo bench --locked -p kernox --bench kernel`
and then this gate in the same job, after the raw benchmark text has been
uploaded as the `kernox-benchmark-<sha>` artifact, so a failing gate still
retains the raw output.

Local gate measurement (2026-09-10, Linux 6.18.18 x86_64, AMD EPYC 9454,
rustc 1.97.1, optimized bench profile, host shared with concurrent builds):

```text
CARGO_TARGET_DIR=$PWD/target cargo bench --locked -p kernox --bench kernel -- steady-state-call
cargo run --locked -p xtask -- bench-budget
```

| Quantity | Value |
| --- | ---: |
| Direct `Arc<dyn Trait>` mean | 1.811175 ns, CI [1.666520, 1.970681] |
| Kernox-extracted mean | 1.404055 ns, CI [1.379346, 1.434997] |
| Delta (point estimates) | −0.224783 |
| Intervals overlap | false |
| Result | pass |

The negative delta is run noise on a shared host (the capability audit measured
roughly a ±2–4% spread for this same comparison), not an acceleration claim:
both paths are the same direct dynamic trait call after boot. The retained
extended-lane measurement artifact
`kernox-benchmark-9a9d3f9037756cbb0345578a9b21a9f55c5aa31b` from schedule run
34080224078 predates this gate; the claim-honesty change records its detailed
numbers. This gate applies the same rule to every scheduled run going forward.

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

## Bounded typed-consumer workload — 2026-08-16

The standalone consumer fixture now exercises the complete typed graph under a
small, reproducible concurrent workload:

```text
cargo run --release --locked --manifest-path fixtures/clean-consumer/Cargo.toml -- --workload
```

Four threads each issue 512 calls through the exported `Application` handle;
the fixture checks every `hello, Kernox @42ms` result and records one duration
per call. A local development snapshot on Linux 6.18.18 x86_64, AMD EPYC 9454,
rustc 1.97.1 in the optimized release profile reported 2,048 calls at p50
`60 ns`, p95 `110 ns`, p99 `130 ns`, and max `3,395 ns`. The verification
command enforces a deliberately broad p99 guardrail of 5 ms and a max-call
guardrail of 100 ms: these catch gross regressions in the real consumer path
without pretending to be an SLA.

## What this does not prove

Kernox is not yet “optimized to the limit.” The evidence proves that the
post-composition call shape is direct typed-handle dispatch and that the tested
control-plane paths and this bounded consumer workload are bounded on one
machine. It does not yet characterize allocator profiles, cold-start
distributions, sustained high-concurrency contention, provider I/O, end-to-end
application latency, cache behavior across CPUs, or tail latency under
sustained load. Those are explicit follow-up measurements before any stable
1.0 decision; optimization work should follow a measured bottleneck instead of
adding speculative machinery to the core.
