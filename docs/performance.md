# Performance evidence

Performance is an acceptance contract, not a permanent claim from one machine.
The scheduled benchmark workflow reruns the same source benchmark and retains
its raw output. Changes to graph resolution, typed provision lookup, or handle
shape must be re-measured.

## Steady-state budget gate — 2026-09-10

The steady-state comparison is enforced as a fail-closed gate over repeated
recorded runs, not as a single observation. The scheduled extended lane keeps
its full `cargo bench --locked -p kernox --bench kernel` distributions run and
raw `benchmark.txt` artifact, then additionally records five fresh steady-state
invocations and gates on them:

1. `cargo run --locked -p xtask -- bench-budget-record --runs 5` resolves the
   Cargo target directory (`cargo metadata --locked --format-version 1
   --no-deps`, `target_directory`, so a configured `CARGO_TARGET_DIR` is
   honored), deletes `<target_directory>/criterion/steady-state-call` so no
   earlier recording can survive, and runs
   `cargo bench --locked -p kernox --bench kernel -- steady-state-call --save-baseline run1..run5`.
2. `cargo run --locked -p xtask -- bench-budget --not-before <start>` reads
   `<target_directory>/criterion/steady-state-call/<benchmark>/run<i>/estimates.json`
   for `i = 1..5` on both benchmarks (all ten files are required).

Both steps run after the evidence upload, so a failing gate still retains
`benchmark.txt`.

Enforced rule (per-side minimum: loading can only add time, so the fastest
observed run estimates each side's clean execution, and a genuine regression
raises every Kernox run):

```text
direct_min = min(direct_point_1..direct_point_5)
kernox_min = min(kernox_point_1..kernox_point_5)
delta = (kernox_min - direct_min) / direct_min
PASS iff delta <= 0.02      # exactly 2% passes
FAIL iff delta > 0.02       # non-zero exit
```

The gate prints every per-run point estimate, interval, and delta, plus
`bench-budget.direct.min`, `bench-budget.kernox.min`, `bench-budget.delta`
(the enforced statistic), `bench-budget.delta.median`, `.min`, `.max`,
`.spread` (the per-run delta spread), `bench-budget.overlap`, and
`bench-budget.result=pass|fail`. It reads only `mean.point_estimate` and
`mean.confidence_interval.lower_bound` / `upper_bound` in ns; the console
`time:` line Criterion prints is its slope estimate. Interval overlap
(closed-interval intersection of the observed ranges) is informational and
never waives the point-estimate budget.

Same-run binding. Every file must have been modified at or after the
invocation start passed as `--not-before`, and the modification times must form
the strict interleaved chain `direct1 < kernox1 < direct2 < kernox2 < ...
< direct5 < kernox5` produced by the five sequential invocations. A stale tree
from an earlier invocation, a fresh direct side paired with a stale Kernox side
(or vice versa), or a reordered recording fails closed instead of being
averaged in.

Fail-closed (non-zero exit, message naming the path and field): missing run or
estimate file, unparsable JSON, missing/non-numeric/non-finite field,
non-positive point estimate or interval bound, `lower_bound > upper_bound`,
stale or out-of-order recordings, and fewer than three recorded runs.

Local gate measurement (2026-09-10, Linux 6.18.18 x86_64, AMD EPYC 9454,
rustc 1.97.1, optimized bench profile, host shared with concurrent builds):

```text
start=$(date -u +%s)
cargo run --locked -p xtask -- bench-budget-record --runs 5
cargo run --locked -p xtask -- bench-budget --not-before "$start"
```

| Run | Direct mean (ns) | Kernox mean (ns) | Delta |
| --- | ---: | ---: | ---: |
| 1 | 2.147497 | 1.386574 | −0.354330 |
| 2 | 1.480936 | 1.682971 | +0.136424 |
| 3 | 1.631315 | 1.404281 | −0.139172 |
| 4 | 1.404451 | 1.654398 | +0.177968 |
| 5 | 1.409471 | 1.481286 | +0.050952 |
| Per-side minimum | 1.404451 | 1.386574 | **−0.012728 (pass)** |

The five per-run deltas spanned 0.532298 (median +0.050952) on this loaded
shared host: a single run cannot decide this budget, and none of these runs is
an acceleration claim. The observed lane spread from the dispatched extended
run is recorded below. The retained extended-lane measurement artifact
`kernox-benchmark-9a9d3f9037756cbb0345578a9b21a9f55c5aa31b` from schedule run
34080224078 predates this gate; the claim-honesty change records its detailed
numbers.

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
