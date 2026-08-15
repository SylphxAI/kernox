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
