# Performance evidence

Performance is an acceptance contract, not a permanent claim from one machine.
The scheduled benchmark workflow reruns the same source benchmark and retains
its raw output. Changes to graph resolution, typed provision lookup, or handle
shape must be re-measured.

## Steady-state budget gate — 2026-09-11

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

Enforced rule: the paired median over recordings. Both benchmarks of one
recording share the same invocation, warmup, and machine state, so a systematic
change to the Kernox path moves every per-recording delta and their median:

```text
direct_i  = direct mean.point_estimate of recording i     (ns)
kernox_i  = Kernox mean.point_estimate of recording i     (ns)
delta_i   = (kernox_i - direct_i) / direct_i
median    = median(delta_1..delta_N)      # N >= 5; the lane records N = 5
pass iff median <= 0.02                   # exactly +2% passes
fail iff median > 0.02                    # non-zero exit
```

The comparison is inclusive: an exactly +2% paired median passes and +2.001%
fails (unit tests and synthetic-tree CLI checks record both). The gate prints
every per-run point estimate, interval, and delta, plus
`bench-budget.direct.min`, `bench-budget.kernox.min`,
`bench-budget.min_side.delta` (the superseded per-side-minimum statistic,
informational only), `bench-budget.delta` (the enforced paired median),
`bench-budget.delta.median`, `.min`, `.max`, `.spread` (the per-run delta
spread), `bench-budget.overlap`, and `bench-budget.result=pass|fail`. It reads
only `mean.point_estimate` and `mean.confidence_interval.lower_bound` /
`upper_bound` in ns; the console `time:` line Criterion prints is its slope
estimate. Interval overlap (closed-interval intersection of the observed
ranges) is informational and never waives the point-estimate budget.

Freshness and ordering check, not proof. Every file must have been modified at
or after the invocation start passed as `--not-before`, and the modification
times must form the strict interleaved chain `direct1 < kernox1 < direct2 <
kernox2 < ... < direct5 < kernox5` produced by the five sequential invocations.
This catches an accidentally stale tree from an earlier invocation, a fresh
direct side paired with a stale Kernox side (or vice versa), or a reordered
recording instead of silently averaging it in. The check trusts the caller to
pass the real invocation start and trusts filesystem modification times:
`--not-before 0`, or copied/`touch`ed mtimes, waive it. It is a stale-evidence
guard, not a tamper-proof binding to the recorded process.

Fail-closed (non-zero exit, message naming the path and field): missing run or
estimate file, unparsable JSON, missing/non-numeric/non-finite field,
non-positive point estimate or interval bound, `lower_bound > upper_bound`,
stale or out-of-order recordings, and fewer than five recorded runs (both the
recorder and the gate refuse N < 5).

### Measured repeatability of the enforced statistic — 2026-09-11

Eight full record-and-decide cycles on unchanged source (host `desk-0`, Linux
6.18.18-talos x86_64, AMD EPYC 9454, rustc 1.97.1, optimized bench profile,
shared with concurrent builds; the desk host, not a `sylphx-linux-standard`
runner) produced these enforced paired medians:

| Cycle | Per-run deltas | Paired median (enforced) | Result |
| ---: | --- | ---: | --- |
| 1 | -0.6864, -1.6314, +0.7878, +1.3660, +0.6470 % | +0.6470 % | pass |
| 2 | +1.2783, -20.0891, +1.0905, +4.1864, -0.7181 % | +1.0905 % | pass |
| 3 | +0.1279, +2.9172, -0.8339, +1.0801, -1.6055 % | +0.1279 % | pass |
| 4 | +0.5250, +0.6394, +0.3897, -1.3532, +0.1399 % | +0.3897 % | pass |
| 5 | +0.4903, +0.8058, -1.3299, +1.1580, -4.7485 % | +0.4903 % | pass |
| 6 | -4.2453, +0.4648, -3.5986, +7.9017, -0.4433 % | -0.4433 % | pass |
| 7 | +2.2873, -0.9584, -4.7201, -0.5051, +15.2935 % | -0.5051 % | pass |
| 8 | +1.4313, -24.3869, -0.1880, +1.5693, +0.6276 % | +0.6276 % | pass |

The enforced median ranged from -0.5051 % to +1.0905 % across these eight
unchanged-source cycles: a repeatability span of 1.5956 percentage points. The
reviewer's five retained clean recordings of the same unchanged source (per-run
deltas in `/tmp/rev46/real-cycles-*`, paired medians +0.5392 %, -0.0452 %,
-1.3080 %, +1.3208 %, +0.5891 %) extend the observed clean range to
[-1.3080 %, +1.3208 %], a span of 2.6288 pp over 13 clean recordings. The
per-run spreads were much larger (0.0199 to 0.2596 on the desk host; 0.043620 on
the dispatched run below), because one loaded run can move a single delta by
tens of percent; the median absorbs those outliers.

These spans are observed samples from specific windows, not bounds on the
variability: a later six-cycle window on the same desk host under heavier load
spanned -2.2088 % to +3.0563 % and included one unchanged-source recording whose
median failed the gate (`+3.0563 %`), which is why a failing gate requires a
clean re-recording rather than a product regression claim.

Two dispatched extended-lane runs of this revision on the self-hosted
`sylphx-linux-standard` runner (2026-09-11) recorded the same protocol and
passed: run
[34550289322](https://github.com/SylphxAI/kernox/actions/runs/34550289322)
produced per-run deltas -0.1720, -1.0916, +1.0272, +0.9643, and -0.3780 %
(paired median **-0.1720 %**, per-run spread 0.021188, informational min-side
-0.1422 %), and run
[34551164171](https://github.com/SylphxAI/kernox/actions/runs/34551164171)
produced -0.6297, -2.7033, -2.3940, -0.3343, and +1.7981 % (paired median
**-0.6297 %**, spread 0.045014, min-side -0.6276 %). Both are consistent with
the desk-host clean range, but two runner samples cannot measure the runner
class's repeatability; the desk-host calibration above is the measured
repeatability claim, and the runner evidence is reported as two passing
recordings only.

### What the gate does and does not detect

The measured repeatability span (1.60 pp on the desk host, 2.63 pp across the 13
clean recordings) is comparable to or larger than the 2% budget, and each
recording uses only five invocations. The gate therefore cannot reliably affirm
that a change is within 2%: a clean recording whose paired median lands above
+2% would fail, and a genuine uniform regression can still pass when the
underlying clean median is low. Concretely, scaling every Kernox point of a
recording by a factor `r` maps its paired median `m` to `r*m + (r - 1)`; on a
clean recording whose median was -1.3080 % (the reviewer's cycle F), a uniform
+2.5 % regression would have produced about +1.16 % and passed, and a uniform
+3.0 % regression about +1.65 % and also passed. The gate does detect systematic
regressions whose shifted median stays above 2% with margin: the reviewer's two
regression counterfactuals fail: a uniform +2.5 % Kernox slowdown derived from
the near-zero-median dispatched recording fails at +2.0769 %, and a constructed
all-runs +3.0 % systematic tree fails at +3.0000 %. It also fails
closed on missing, malformed, stale, mixed, or reordered evidence. Treat a
failing gate as a red flag that needs a clean re-recording and investigation;
treat a passing gate as evidence that no regression larger than the measured
repeatability was observed in that recording, not as a 2% measurement.

### Superseded per-side-minimum statistic and counterfactual evidence

The first revision of this gate decided on
`(min(kernox) - min(direct)) / min(direct)`. On trees derived from the author's
own dispatched recording (run
[34536513077](https://github.com/SylphxAI/kernox/actions/runs/34536513077)), a
uniform +2.5 % systematic regression on every Kernox point produced the min-side
statistic 0.016786 (pass) while the paired median was +0.020769; an all-runs
+3.0 % systematic regression produced the min-side statistic 0.019000 (pass)
while the paired median was +0.030000. Under the enforced paired-median rule
both fail (`0.020769 > 0.02`, `0.030000 > 0.02`).

Dispatched extended-lane measurement (run
[34536513077](https://github.com/SylphxAI/kernox/actions/runs/34536513077),
benchmark job `success`, revision
`c90e0bdf3dae6f835e4c7f3361b99ce1bbe6a606`, self-hosted Linux runner,
2026-09-10): the five per-run deltas were -0.003481, -0.004128, -0.009412,
-0.034149, and +0.009471 (paired median -0.004128; per-run spread 0.043620).
That recording passes the paired-median budget, and its per-run spread of
0.043620 measures the delta spread within that one recording, not the
repeatability of the enforced statistic. The retained extended-lane measurement
artifact `kernox-benchmark-9a9d3f9037756cbb0345578a9b21a9f55c5aa31b` from
schedule run 34080224078 predates this gate; the claim-honesty change records
its detailed numbers.

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

## Extended-lane steady-state benchmark — 2026-09-07

The scheduled extended lane reruns the same Criterion benchmark on a platform
runner and retains the raw output as a build artifact. The retained artifact
`kernox-benchmark-9a9d3f9037756cbb0345578a9b21a9f55c5aa31b` (schedule run
`34080224078`, commit `9a9d3f9`, benchmark job success at 2026-09-07T03:41:52Z)
was downloaded with `gh run download 34080224078 --name
kernox-benchmark-9a9d3f9037756cbb0345578a9b21a9f55c5aa31b`; the unpacked
`benchmark.txt` has sha256
`575dd60b7ed4b1dfbfe47dc698c44a59bfee2c654ffc3d14e63bbd20abf20e28`.

Command and environment:

```text
cargo bench --locked -p kernox --bench kernel
```

Criterion defaults (100 samples per steady-state benchmark) under the pinned
1.97.1 toolchain on the `sylphx-linux-standard` runner. Criterion printed
exactly:

```text
steady-state-call/direct-arc-dyn-trait
                        time:   [1.3314 ns 1.3369 ns 1.3440 ns]
steady-state-call/kernox-extracted-arc-dyn-trait
                        time:   [1.3265 ns 1.3287 ns 1.3314 ns]
```

The console `time:` line Criterion 0.8.2 prints is rendered from its
`typical()` estimate, which is the slope estimate when one is available and the
mean otherwise (`criterion-0.8.2/src/estimate.rs`); the values above are those
console values. The steady-state budget gate reads `mean.point_estimate` from
Criterion's machine-readable `estimates.json` instead. The retained artifact
holds only console text, so the comparison below is stated on the console values
and does not by itself assert the mean-based gate decision for this run.

| Path | Console `time:` estimate | 95% confidence interval |
| --- | ---: | ---: |
| Direct `Arc<dyn Trait>` call | 1.3369 ns | 1.3314–1.3440 ns |
| Kernox-extracted `Arc<dyn Trait>` call | 1.3287 ns | 1.3265–1.3314 ns |

The point-estimate delta on the recorded console values is approximately −0.61%
(Kernox-extracted below direct). The printed confidence intervals share only the
endpoint 1.3314 ns and do not otherwise overlap. The declared 2% budget holds on
this observation.

Limits: this is one scheduled observation at Criterion's default sampling; the
artifact does not record the runner's machine class. The 2026-08-16 development
matrix above records a roughly 2% spread between steady-state runs on the desk
machine (Linux 6.18.18 x86_64, AMD EPYC 9454), which is not the extended-lane
runner. A single Criterion comparison is therefore not a stable contract, and
this section records the budget comparison for that one run rather than a
distribution-level guarantee.

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
