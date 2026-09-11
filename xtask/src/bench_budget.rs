//! Fail-closed steady-state budget gate over repeated Criterion runs.
//!
//! A single Criterion comparison on a shared machine cannot discriminate a real
//! 2% regression from run-to-run noise, so the gate deliberately records
//! repeated paired invocations and decides on the median of the per-recording
//! paired deltas. `bench-budget-record` deletes the recorded
//! `steady-state-call` results and runs the benchmark `N` times (default 5,
//! minimum 5) with `--save-baseline run1..runN`. The gate then reads both
//! benchmarks' `run<i>/estimates.json` for every `i` and pairs them by
//! recording:
//!
//! ```text
//! delta_i = (kernox_point_i - direct_point_i) / direct_point_i
//! statistic = median(delta_1..delta_N)      # paired median delta
//! PASS iff statistic <= 0.02                # exactly 2% passes
//! ```
//!
//! Both benchmarks of one recording share the same invocation, warmup, and
//! machine state, so a systematic change to the Kernox path moves every
//! `delta_i` and the median. The per-run deltas, their median, and their spread
//! are printed, and the median is the enforced `bench-budget.delta`. The
//! per-side minimum delta of the previous min-side rule is printed as
//! `bench-budget.min_side.delta` for inspection only and does not decide.
//!
//! Freshness and ordering check (not a proof): the gate requires every
//! `estimates.json` to be modified at or after the caller-supplied
//! `--not-before` epoch and the modification times to form the strict
//! interleaved order `direct1 < kernox1 < direct2 < kernox2 < ... `. Every one
//! of the `2 * N` files is required. This catches an accidentally stale, mixed,
//! or reordered tree instead of silently averaging it in, but it trusts the
//! caller to pass the real invocation start and trusts the filesystem
//! modification times: `--not-before 0` or touched/copied mtimes waive the
//! check, so it is not a tamper-proof binding to the recorded process.
//!
//! The point estimates decide; the `mean.confidence_interval` ranges are
//! reported as overlapping or not, and that overlap never waives the budget.
//! Missing files, unparsable JSON, and missing, non-finite, non-positive, or
//! out-of-order fields fail closed with a message naming the path and field.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Declared acceptance budget for the median paired steady-state delta.
pub(crate) const STEADY_STATE_BUDGET: f64 = 0.02;

/// Default number of recorded benchmark invocations in the scheduled lane.
pub(crate) const DEFAULT_RUNS: usize = 5;

/// Fewer than five paired runs cannot support a median decision, so the gate
/// refuses them.
pub(crate) const MIN_RUNS: usize = 5;

/// Criterion benchmark path of the direct `Arc<dyn Trait>` control call.
pub(crate) const DIRECT_BENCHMARK: &str = "steady-state-call/direct-arc-dyn-trait";

/// Criterion benchmark path of the Kernox-extracted `Arc<dyn Trait>` call.
pub(crate) const KERNOX_BENCHMARK: &str = "steady-state-call/kernox-extracted-arc-dyn-trait";

const STEADY_STATE_GROUP: &str = "steady-state-call";
const ESTIMATES_FILE: &str = "estimates.json";
const BASELINE_PREFIX: &str = "run";

/// A Criterion mean estimate and its confidence interval, in nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Estimate {
    pub(crate) point: f64,
    pub(crate) lower: f64,
    pub(crate) upper: f64,
}

/// One recorded benchmark invocation: both sides plus the modification times
/// that bind the pair to a single invocation window.
#[derive(Debug, Clone)]
pub(crate) struct RunSample {
    pub(crate) direct: Estimate,
    pub(crate) kernox: Estimate,
    pub(crate) direct_path: PathBuf,
    pub(crate) kernox_path: PathBuf,
    pub(crate) direct_modified: SystemTime,
    pub(crate) kernox_modified: SystemTime,
}

/// The aggregated budget decision over the recorded runs.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RunsOutcome {
    pub(crate) deltas: Vec<f64>,
    /// Enforced statistic: the median of the per-recording paired deltas.
    pub(crate) median_delta: f64,
    /// Informational min-side delta `(min(kernox) - min(direct)) / min(direct)`.
    pub(crate) min_side_delta: f64,
    pub(crate) min_delta: f64,
    pub(crate) max_delta: f64,
    pub(crate) overlap: bool,
    pub(crate) pass: bool,
    pub(crate) direct_min: f64,
    pub(crate) kernox_min: f64,
    pub(crate) direct_point_median: f64,
    pub(crate) kernox_point_median: f64,
    pub(crate) direct_lower: f64,
    pub(crate) direct_upper: f64,
    pub(crate) kernox_lower: f64,
    pub(crate) kernox_upper: f64,
}

impl Estimate {
    /// Builds a validated estimate: every value must be finite, positive, and
    /// the interval bounds must be ordered (measured times are positive ns).
    fn new(point: f64, lower: f64, upper: f64) -> Result<Self, String> {
        if !point.is_finite() {
            return Err("field mean.point_estimate is not finite".to_owned());
        }
        if !lower.is_finite() {
            return Err("field mean.confidence_interval.lower_bound is not finite".to_owned());
        }
        if !upper.is_finite() {
            return Err("field mean.confidence_interval.upper_bound is not finite".to_owned());
        }
        if point <= 0.0 {
            return Err("field mean.point_estimate is not positive".to_owned());
        }
        if lower <= 0.0 {
            return Err("field mean.confidence_interval.lower_bound is not positive".to_owned());
        }
        if upper <= 0.0 {
            return Err("field mean.confidence_interval.upper_bound is not positive".to_owned());
        }
        if lower > upper {
            return Err("field mean.confidence_interval.lower_bound exceeds upper_bound".to_owned());
        }
        Ok(Self { point, lower, upper })
    }
}

/// Parses a Criterion `estimates.json` document, naming `path` and the exact
/// JSON field in every failure so a malformed run cannot pass silently.
pub(crate) fn parse_estimate(source: &str, path: &Path) -> Result<Estimate, String> {
    let document: serde_json::Value = serde_json::from_str(source)
        .map_err(|error| format!("{}: unparsable estimates JSON: {error}", path.display()))?;
    let mean =
        document.get("mean").ok_or_else(|| format!("{}: missing field mean", path.display()))?;
    let point = finite_field(mean, "point_estimate", path)?;
    let interval = mean
        .get("confidence_interval")
        .ok_or_else(|| format!("{}: missing field mean.confidence_interval", path.display()))?;
    let lower = finite_field(interval, "lower_bound", path)?;
    let upper = finite_field(interval, "upper_bound", path)?;
    Estimate::new(point, lower, upper).map_err(|error| format!("{}: {error}", path.display()))
}

fn finite_field(value: &serde_json::Value, field: &str, path: &Path) -> Result<f64, String> {
    let number = value
        .get(field)
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| format!("{}: field {field} is missing or not a number", path.display()))?;
    if !number.is_finite() {
        return Err(format!("{}: field {field} is not finite", path.display()));
    }
    Ok(number)
}

/// Aggregates the recorded runs: refuses too few, stale, mixed, or out-of-order
/// recordings, then enforces the declared budget on the median of the
/// per-recording paired point-estimate deltas.
pub(crate) fn evaluate_runs(
    samples: &[RunSample],
    not_before: SystemTime,
) -> Result<RunsOutcome, String> {
    if samples.is_empty() {
        return Err("no recorded benchmark runs to evaluate".to_owned());
    }
    if samples.len() < MIN_RUNS {
        return Err(format!(
            "at least {MIN_RUNS} recorded runs are required for a median decision; got {}",
            samples.len()
        ));
    }
    for (index, sample) in samples.iter().enumerate() {
        let run = index + 1;
        if sample.direct_modified < not_before {
            return Err(format!(
                "{} was modified before the recorded invocation start; stale input",
                sample.direct_path.display()
            ));
        }
        if sample.kernox_modified < not_before {
            return Err(format!(
                "{} was modified before the recorded invocation start; stale input",
                sample.kernox_path.display()
            ));
        }
        if sample.kernox_modified <= sample.direct_modified {
            return Err(format!(
                "{} is not newer than {}; run {run} is not one ordered invocation",
                sample.kernox_path.display(),
                sample.direct_path.display()
            ));
        }
        if let Some(previous) = index.checked_sub(1).map(|previous| &samples[previous]) {
            if sample.direct_modified <= previous.kernox_modified {
                return Err(format!(
                    "{} is not newer than {}; run {run} was not recorded after run {index}",
                    sample.direct_path.display(),
                    previous.kernox_path.display()
                ));
            }
        }
    }
    let mut deltas = Vec::with_capacity(samples.len());
    for sample in samples {
        let delta = (sample.kernox.point - sample.direct.point) / sample.direct.point;
        if !delta.is_finite() {
            return Err(format!(
                "computed delta is not finite for {} and {}",
                sample.direct_path.display(),
                sample.kernox_path.display()
            ));
        }
        deltas.push(delta);
    }
    let direct_min = samples.iter().map(|sample| sample.direct.point).fold(f64::INFINITY, f64::min);
    let kernox_min = samples.iter().map(|sample| sample.kernox.point).fold(f64::INFINITY, f64::min);
    let min_side_delta = (kernox_min - direct_min) / direct_min;
    if !min_side_delta.is_finite() {
        return Err(format!("computed min-side delta is not finite (direct minimum {direct_min})"));
    }
    let median_delta = median(&deltas);
    if !median_delta.is_finite() {
        return Err("computed median paired delta is not finite".to_owned());
    }
    let min_delta = deltas.iter().copied().fold(f64::INFINITY, f64::min);
    let max_delta = deltas.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let direct_points: Vec<f64> = samples.iter().map(|sample| sample.direct.point).collect();
    let kernox_points: Vec<f64> = samples.iter().map(|sample| sample.kernox.point).collect();
    let direct_lower =
        samples.iter().map(|sample| sample.direct.lower).fold(f64::INFINITY, f64::min);
    let direct_upper =
        samples.iter().map(|sample| sample.direct.upper).fold(f64::NEG_INFINITY, f64::max);
    let kernox_lower =
        samples.iter().map(|sample| sample.kernox.lower).fold(f64::INFINITY, f64::min);
    let kernox_upper =
        samples.iter().map(|sample| sample.kernox.upper).fold(f64::NEG_INFINITY, f64::max);
    Ok(RunsOutcome {
        deltas,
        median_delta,
        min_side_delta,
        min_delta,
        max_delta,
        // Closed-interval intersection of the observed range of each side;
        // touching endpoints still count as overlap.
        overlap: direct_lower <= kernox_upper && kernox_lower <= direct_upper,
        pass: median_delta <= STEADY_STATE_BUDGET,
        direct_min,
        kernox_min,
        direct_point_median: median(&direct_points),
        kernox_point_median: median(&kernox_points),
        direct_lower,
        direct_upper,
        kernox_lower,
        kernox_upper,
    })
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        f64::midpoint(sorted[middle - 1], sorted[middle])
    } else {
        sorted[middle]
    }
}

/// Location of the recorded estimates for one Criterion benchmark and run.
pub(crate) fn estimate_path(criterion_dir: &Path, benchmark: &str, run: usize) -> PathBuf {
    criterion_dir.join(benchmark).join(format!("{BASELINE_PREFIX}{run}")).join(ESTIMATES_FILE)
}

fn read_estimate(path: &Path) -> Result<(Estimate, SystemTime), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    let modified = metadata.modified().map_err(|error| {
        format!("could not read the modification time of {}: {error}", path.display())
    })?;
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    Ok((parse_estimate(&source, path)?, modified))
}

/// Criterion directory selection: the resolved Cargo target directory by
/// default, or an explicit `--criterion-dir` override.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CriterionDir {
    Default,
    Explicit(PathBuf),
}

/// Parsed `bench-budget` invocation: how many recorded runs, where, and the
/// invocation start that binds the recording to this gate call.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GateOptions {
    pub(crate) criterion_dir: CriterionDir,
    pub(crate) runs: usize,
    pub(crate) not_before: SystemTime,
}

/// Parsed `bench-budget-record` invocation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RecordOptions {
    pub(crate) criterion_dir: CriterionDir,
    pub(crate) runs: usize,
}

/// Parses `bench-budget [--criterion-dir DIR] [--runs N] --not-before EPOCH`.
/// `--not-before` is required: without the invocation start there is no way to
/// guard against stale recordings, so the gate refuses to run.
pub(crate) fn parse_gate_arguments(arguments: &[String]) -> Result<GateOptions, String> {
    let mut criterion_dir = CriterionDir::Default;
    let mut runs = DEFAULT_RUNS;
    let mut not_before = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--criterion-dir" => {
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| "bench-budget expects --criterion-dir DIR".to_owned())?;
                criterion_dir = CriterionDir::Explicit(PathBuf::from(value));
                index += 2;
            }
            "--runs" => {
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| "bench-budget expects --runs COUNT".to_owned())?;
                runs = value
                    .parse::<usize>()
                    .map_err(|error| format!("bench-budget --runs is not a count: {error}"))?;
                index += 2;
            }
            "--not-before" => {
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| "bench-budget expects --not-before EPOCH_SECONDS".to_owned())?;
                let epoch = value.parse::<u64>().map_err(|error| {
                    format!("bench-budget --not-before is not an epoch in seconds: {error}")
                })?;
                not_before = Some(UNIX_EPOCH + Duration::from_secs(epoch));
                index += 2;
            }
            argument => return Err(format!("unknown bench-budget argument {argument}")),
        }
    }
    let not_before = not_before.ok_or_else(|| {
        "bench-budget requires --not-before EPOCH_SECONDS recorded before bench-budget-record ran"
            .to_owned()
    })?;
    check_runs(runs)?;
    Ok(GateOptions { criterion_dir, runs, not_before })
}

/// Parses `bench-budget-record [--criterion-dir DIR] [--runs N]`.
pub(crate) fn parse_record_arguments(arguments: &[String]) -> Result<RecordOptions, String> {
    let mut criterion_dir = CriterionDir::Default;
    let mut runs = DEFAULT_RUNS;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--criterion-dir" => {
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| "bench-budget-record expects --criterion-dir DIR".to_owned())?;
                criterion_dir = CriterionDir::Explicit(PathBuf::from(value));
                index += 2;
            }
            "--runs" => {
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| "bench-budget-record expects --runs COUNT".to_owned())?;
                runs = value.parse::<usize>().map_err(|error| {
                    format!("bench-budget-record --runs is not a count: {error}")
                })?;
                index += 2;
            }
            argument => return Err(format!("unknown bench-budget-record argument {argument}")),
        }
    }
    check_runs(runs)?;
    Ok(RecordOptions { criterion_dir, runs })
}

fn check_runs(runs: usize) -> Result<(), String> {
    if runs < MIN_RUNS {
        return Err(format!(
            "at least {MIN_RUNS} recorded runs are required so the paired median is meaningful; got {runs}"
        ));
    }
    Ok(())
}

/// Resolves the selection: an explicit directory is used as-is; otherwise the
/// Cargo target directory is read from `cargo metadata`, so a configured
/// `CARGO_TARGET_DIR` (or any other target-directory configuration) is honored
/// instead of assuming a relative `target` directory.
pub(crate) fn resolve_criterion_dir(selection: &CriterionDir) -> Result<PathBuf, String> {
    match selection {
        CriterionDir::Explicit(criterion_dir) => Ok(criterion_dir.clone()),
        CriterionDir::Default => {
            let output = Command::new("cargo")
                .args(["metadata", "--locked", "--format-version", "1", "--no-deps"])
                .output()
                .map_err(|error| {
                    format!("could not run cargo metadata to resolve the target directory: {error}")
                })?;
            if !output.status.success() {
                return Err(format!(
                    "cargo metadata failed while resolving the target directory: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            let source = std::str::from_utf8(&output.stdout)
                .map_err(|error| format!("cargo metadata output is not UTF-8: {error}"))?;
            target_directory_from_metadata(source).map(|target| target.join("criterion"))
        }
    }
}

/// Reads the absolute `target_directory` that `cargo metadata` reports for
/// this workspace and machine configuration.
pub(crate) fn target_directory_from_metadata(source: &str) -> Result<PathBuf, String> {
    let document: serde_json::Value = serde_json::from_str(source)
        .map_err(|error| format!("unparsable cargo metadata: {error}"))?;
    let target_directory = document
        .get("target_directory")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cargo metadata is missing a string target_directory".to_owned())?;
    Ok(PathBuf::from(target_directory))
}

/// Records `runs` fresh steady-state invocations under `criterion_dir`,
/// clearing the previous `steady-state-call` results first so no stale run can
/// satisfy the gate.
pub(crate) fn run_record(options: &RecordOptions) -> Result<(), String> {
    let criterion_dir = resolve_criterion_dir(&options.criterion_dir)?;
    let steady_state = criterion_dir.join(STEADY_STATE_GROUP);
    match fs::remove_dir_all(&steady_state) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!("could not clear {}: {error}", steady_state.display()));
        }
    }
    println!("bench-budget-record.criterion-dir={}", criterion_dir.display());
    println!("bench-budget-record.runs={}", options.runs);
    for run in 1..=options.runs {
        let baseline = format!("{BASELINE_PREFIX}{run}");
        let status = Command::new("cargo")
            .args([
                "bench",
                "--locked",
                "-p",
                "kernox",
                "--bench",
                "kernel",
                "--",
                STEADY_STATE_GROUP,
                "--save-baseline",
                &baseline,
            ])
            .status()
            .map_err(|error| format!("could not run benchmark run {run}: {error}"))?;
        if !status.success() {
            return Err(format!("benchmark run {run} exited with {status}"));
        }
    }
    Ok(())
}

/// Runs the gate: reads every recorded run, checks freshness and ordering,
/// prints the machine-readable report, and fails when the median paired delta
/// exceeds the declared budget or the evidence is unusable.
pub(crate) fn run(options: &GateOptions) -> Result<(), String> {
    let criterion_dir = resolve_criterion_dir(&options.criterion_dir)?;
    let mut samples = Vec::with_capacity(options.runs);
    for run in 1..=options.runs {
        let direct_path = estimate_path(&criterion_dir, DIRECT_BENCHMARK, run);
        let (direct, direct_modified) = read_estimate(&direct_path)?;
        let kernox_path = estimate_path(&criterion_dir, KERNOX_BENCHMARK, run);
        let (kernox, kernox_modified) = read_estimate(&kernox_path)?;
        samples.push(RunSample {
            direct,
            kernox,
            direct_path,
            kernox_path,
            direct_modified,
            kernox_modified,
        });
    }
    let outcome = evaluate_runs(&samples, options.not_before)?;
    print_report(&criterion_dir, options.runs, &samples, &outcome);
    if outcome.pass {
        Ok(())
    } else {
        Err(format!(
            "median paired steady-state point-estimate delta {:.6} exceeds declared budget {:.6}",
            outcome.median_delta, STEADY_STATE_BUDGET
        ))
    }
}

fn print_report(criterion_dir: &Path, runs: usize, samples: &[RunSample], outcome: &RunsOutcome) {
    println!("bench-budget.mode=runs");
    println!("bench-budget.runs={runs}");
    println!("bench-budget.criterion-dir={}", criterion_dir.display());
    println!("bench-budget.statistic=paired-median-delta");
    for (index, sample) in samples.iter().enumerate() {
        let run = index + 1;
        println!("bench-budget.run{run}.direct={:.6}", sample.direct.point);
        println!("bench-budget.run{run}.kernox={:.6}", sample.kernox.point);
        println!(
            "bench-budget.run{run}.direct.interval=[{:.6}, {:.6}]",
            sample.direct.lower, sample.direct.upper
        );
        println!(
            "bench-budget.run{run}.kernox.interval=[{:.6}, {:.6}]",
            sample.kernox.lower, sample.kernox.upper
        );
        println!("bench-budget.run{run}.delta={:.6}", outcome.deltas[index]);
    }
    println!("bench-budget.direct.min={:.6}", outcome.direct_min);
    println!("bench-budget.kernox.min={:.6}", outcome.kernox_min);
    println!("bench-budget.min_side.delta={:.6}", outcome.min_side_delta);
    println!("bench-budget.direct.point_median={:.6}", outcome.direct_point_median);
    println!("bench-budget.kernox.point_median={:.6}", outcome.kernox_point_median);
    println!(
        "bench-budget.direct.interval=[{:.6}, {:.6}]",
        outcome.direct_lower, outcome.direct_upper
    );
    println!(
        "bench-budget.kernox.interval=[{:.6}, {:.6}]",
        outcome.kernox_lower, outcome.kernox_upper
    );
    println!("bench-budget.delta={:.6}", outcome.median_delta);
    println!("bench-budget.delta.median={:.6}", outcome.median_delta);
    println!("bench-budget.delta.min={:.6}", outcome.min_delta);
    println!("bench-budget.delta.max={:.6}", outcome.max_delta);
    println!("bench-budget.delta.spread={:.6}", outcome.max_delta - outcome.min_delta);
    println!("bench-budget.overlap={}", outcome.overlap);
    println!("bench-budget.result={}", if outcome.pass { "pass" } else { "fail" });
}

#[cfg(test)]
mod tests {
    // Recorded measurement points are data tables; underscores would obscure
    // the correspondence with the raw logs.
    #![allow(clippy::expect_used, clippy::unreadable_literal, clippy::unwrap_used)]

    use std::{
        path::{Path, PathBuf},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use super::{
        CriterionDir, DEFAULT_RUNS, DIRECT_BENCHMARK, Estimate, MIN_RUNS, RunSample,
        STEADY_STATE_BUDGET, estimate_path, evaluate_runs, parse_estimate, parse_gate_arguments,
        parse_record_arguments, resolve_criterion_dir, target_directory_from_metadata,
    };

    const DIRECT_SOURCE: &str = r#"{
        "mean": {
            "confidence_interval": {
                "confidence_level": 0.95,
                "lower_bound": 99.0,
                "upper_bound": 101.0
            },
            "point_estimate": 100.0,
            "standard_error": 0.5
        },
        "median": { "point_estimate": 100.0 }
    }"#;

    fn epoch(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn sample(run: usize, direct: f64, kernox: f64, start: u64) -> RunSample {
        let offset = u64::try_from(run).expect("small run index") * 100;
        RunSample {
            direct: Estimate { point: direct, lower: direct * 0.99, upper: direct * 1.01 },
            kernox: Estimate { point: kernox, lower: kernox * 0.99, upper: kernox * 1.01 },
            direct_path: estimate_path(Path::new("/tmp/criterion"), DIRECT_BENCHMARK, run),
            kernox_path: estimate_path(Path::new("/tmp/criterion"), super::KERNOX_BENCHMARK, run),
            direct_modified: epoch(start + offset),
            kernox_modified: epoch(start + offset + 10),
        }
    }

    fn runs_with(deltas: [f64; 5]) -> Vec<RunSample> {
        deltas
            .iter()
            .enumerate()
            .map(|(index, delta)| sample(index + 1, 100.0, 100.0 * (1.0 + delta), 1_000))
            .collect()
    }

    fn samples_from_points(direct_points: [f64; 5], kernox_points: [f64; 5]) -> Vec<RunSample> {
        direct_points
            .iter()
            .zip(kernox_points)
            .enumerate()
            .map(|(index, (direct, kernox))| sample(index + 1, *direct, kernox, 1_000))
            .collect()
    }

    #[test]
    fn parses_the_observed_estimates_schema() {
        let parsed = parse_estimate(DIRECT_SOURCE, Path::new("run3/estimates.json"))
            .expect("observed Criterion schema parses");
        assert_eq!(parsed, Estimate { point: 100.0, lower: 99.0, upper: 101.0 });
    }

    #[test]
    fn missing_mean_field_fails_closed() {
        let error = parse_estimate("{}", Path::new("run3/estimates.json")).expect_err("must fail");
        assert!(error.contains("missing field mean"), "{error}");
    }

    #[test]
    fn missing_interval_and_point_fields_name_the_field() {
        let missing_point =
            r#"{"mean":{"confidence_interval":{"lower_bound":1.0,"upper_bound":2.0}}}"#;
        let error =
            parse_estimate(missing_point, Path::new("run1/estimates.json")).expect_err("fails");
        assert!(error.contains("point_estimate"), "{error}");

        let missing_lower =
            r#"{"mean":{"confidence_interval":{"upper_bound":2.0},"point_estimate":1.0}}"#;
        let error =
            parse_estimate(missing_lower, Path::new("run1/estimates.json")).expect_err("fails");
        assert!(error.contains("lower_bound"), "{error}");
    }

    #[test]
    fn unparsable_json_fails_closed_and_names_the_path() {
        let error =
            parse_estimate("not json", Path::new("run1/estimates.json")).expect_err("fails");
        assert!(error.contains("run1/estimates.json"), "{error}");
        assert!(error.contains("unparsable"), "{error}");
    }

    #[test]
    fn non_finite_values_fail_closed() {
        // serde_json rejects out-of-range numeric literals before any value is
        // constructed, so an overflowing estimate is an unparsable document.
        let overflow = r#"{"mean":{"confidence_interval":{"lower_bound":1.0,"upper_bound":2.0},"point_estimate":1e400}}"#;
        let error = parse_estimate(overflow, Path::new("run1/estimates.json")).expect_err("fails");
        assert!(error.contains("run1/estimates.json"), "{error}");

        let nan = Estimate::new(f64::NAN, 1.0, 2.0).expect_err("NaN fails");
        assert!(nan.contains("point_estimate"), "{nan}");
        let nan_bound = Estimate::new(1.0, f64::NAN, 2.0).expect_err("NaN bound fails");
        assert!(nan_bound.contains("lower_bound"), "{nan_bound}");
        let infinite_bound = Estimate::new(1.0, 0.5, f64::INFINITY).expect_err("inf fails");
        assert!(infinite_bound.contains("upper_bound"), "{infinite_bound}");
    }

    #[test]
    fn non_positive_values_fail_closed() {
        let zero_point = Estimate::new(0.0, 0.5, 1.0).expect_err("zero point fails");
        assert!(zero_point.contains("point_estimate"), "{zero_point}");
        let negative_point = Estimate::new(-1.0, 0.5, 1.0).expect_err("negative point fails");
        assert!(negative_point.contains("point_estimate"), "{negative_point}");
        let zero_lower = Estimate::new(1.0, 0.0, 1.0).expect_err("zero lower fails");
        assert!(zero_lower.contains("lower_bound"), "{zero_lower}");
        let negative_lower = Estimate::new(1.0, -0.5, 1.0).expect_err("negative lower fails");
        assert!(negative_lower.contains("lower_bound"), "{negative_lower}");
        let zero_upper = Estimate::new(1.0, 0.5, 0.0).expect_err("zero upper fails");
        assert!(zero_upper.contains("upper_bound"), "{zero_upper}");
        let negative_upper = Estimate::new(1.0, 0.5, -2.0).expect_err("negative upper fails");
        assert!(negative_upper.contains("upper_bound"), "{negative_upper}");
    }

    #[test]
    fn out_of_order_interval_fails_closed() {
        let error = Estimate::new(1.0, 2.0, 1.0).expect_err("lower above upper fails");
        assert!(error.contains("lower_bound exceeds upper_bound"), "{error}");
    }

    #[test]
    fn estimate_paths_match_the_observed_baseline_layout() {
        assert_eq!(
            estimate_path(Path::new("criterion"), DIRECT_BENCHMARK, 3),
            Path::new("criterion/steady-state-call/direct-arc-dyn-trait/run3/estimates.json")
        );
        assert_eq!(
            estimate_path(Path::new("criterion"), super::KERNOX_BENCHMARK, 5),
            Path::new(
                "criterion/steady-state-call/kernox-extracted-arc-dyn-trait/run5/estimates.json"
            )
        );
    }

    #[test]
    fn median_paired_delta_exactly_two_percent_passes() {
        let samples = runs_with([0.02; 5]);
        let outcome = evaluate_runs(&samples, epoch(1_000)).expect("valid recording");
        assert!((outcome.median_delta - STEADY_STATE_BUDGET).abs() < f64::EPSILON);
        assert!(outcome.pass);
    }

    #[test]
    fn median_paired_delta_just_over_two_percent_fails() {
        let samples = runs_with([0.020001; 5]);
        let outcome = evaluate_runs(&samples, epoch(1_000)).expect("valid recording");
        assert!(outcome.median_delta > STEADY_STATE_BUDGET);
        assert!(!outcome.pass);

        let samples = runs_with([0.0201; 5]);
        let outcome = evaluate_runs(&samples, epoch(1_000)).expect("valid recording");
        assert!(outcome.median_delta > STEADY_STATE_BUDGET);
        assert!(!outcome.pass);

        let genuine_regression = runs_with([0.10; 5]);
        let outcome = evaluate_runs(&genuine_regression, epoch(1_000)).expect("valid recording");
        assert!((outcome.median_delta - 0.10).abs() < f64::EPSILON);
        assert!(!outcome.pass);
    }

    #[test]
    fn median_paired_delta_rejects_uniform_systematic_regressions() {
        // Counterfactual from the dispatched CI recording (run 34536513077):
        // a uniform +2.5% systematic regression on every Kernox point turns the
        // per-run deltas into [0.021432, 0.020769, 0.015353, -0.010002,
        // 0.034708]; the paired median +2.0769% must fail the 2% budget.
        let direct = [1.373721, 1.374117, 1.375659, 1.413451, 1.377446];
        let kernox = [1.403162, 1.402656, 1.396780, 1.399313, 1.425254];
        let samples = samples_from_points(direct, kernox);
        let outcome = evaluate_runs(&samples, epoch(1_000)).expect("valid recording");
        assert!((outcome.deltas[0] - 0.021432).abs() < 5e-7, "{:?}", outcome.deltas);
        assert!((outcome.median_delta - 0.020769).abs() < 5e-7, "{}", outcome.median_delta);
        assert!(outcome.median_delta > STEADY_STATE_BUDGET);
        assert!(!outcome.pass, "a +2.5% systematic regression must fail");

        // Counterfactual the reviewer built as an all-runs +3.0% systematic
        // regression: deltas [0.019, 0.026, 0.030, 0.034, 0.038], median +3%.
        let samples = samples_from_points([100.0; 5], [101.9, 102.6, 103.0, 103.4, 103.8]);
        let outcome = evaluate_runs(&samples, epoch(1_000)).expect("valid recording");
        assert!((outcome.median_delta - 0.030).abs() < 1e-12, "{}", outcome.median_delta);
        assert!(!outcome.pass, "a +3.0% systematic regression must fail");
    }

    #[test]
    fn median_paired_delta_passes_reviewer_clean_recordings() {
        // The reviewer's eight clean recordings of unchanged source produced
        // enforced min-side statistics of +1.026%, +1.530%, +0.690%, -0.199%,
        // -0.536%, -1.015%, -0.427%, +0.589%. These are the per-run deltas of
        // the last five (cycles D..H); the paired median of each must pass.
        let cycles: [([f64; 5], f64); 5] = [
            ([0.035791, -0.005386, 0.005392, -0.004068, 0.010163], 0.005392),
            ([0.009099, -0.005406, -0.000452, 0.009239, -0.010499], -0.000452),
            ([0.016505, -0.025796, -0.013080, 0.003634, -0.024798], -0.013080),
            ([-0.000808, 0.013208, 0.017786, 0.013240, -0.012498], 0.013208),
            ([0.005891, 0.006361, 0.009073, -0.009543, -0.429505], 0.005891),
        ];
        for (deltas, expected_median) in cycles {
            let kernox: [f64; 5] = deltas.map(|delta| 100.0 * (1.0 + delta));
            let samples = samples_from_points([100.0; 5], kernox);
            let outcome = evaluate_runs(&samples, epoch(1_000)).expect("valid recording");
            assert!(
                (outcome.median_delta - expected_median).abs() < 5e-7,
                "median {} != {expected_median}",
                outcome.median_delta
            );
            assert!(outcome.pass, "clean recording with deltas {deltas:?} must pass");
        }
    }

    #[test]
    fn median_statistic_ignores_one_loaded_run() {
        // One heavily loaded Kernox run does not fail a clean recording.
        let loaded_kernox = samples_from_points([100.0; 5], [101.0, 101.0, 160.0, 101.0, 101.0]);
        let outcome = evaluate_runs(&loaded_kernox, epoch(1_000)).expect("valid recording");
        assert!((outcome.median_delta - 0.01).abs() < 1e-12);
        assert!((outcome.min_side_delta - 0.01).abs() < 1e-12);
        assert!(outcome.pass);

        // One heavily loaded direct run does not fail a clean recording either.
        let loaded_direct = samples_from_points([100.0, 160.0, 100.0, 100.0, 100.0], [101.0; 5]);
        let outcome = evaluate_runs(&loaded_direct, epoch(1_000)).expect("valid recording");
        assert!((outcome.median_delta - 0.01).abs() < 1e-12);
        assert!(outcome.pass);

        // The per-run spread stays visible even when the statistic passes.
        assert!((outcome.max_delta - outcome.min_delta - 0.37875).abs() < 1e-12);
    }

    #[test]
    fn median_statistic_fails_on_a_consistent_regression() {
        // Every Kernox run is at least 3.5% slow; the median decides.
        let samples = runs_with([0.03, 0.05, 0.04, 0.06, 0.035]);
        let outcome = evaluate_runs(&samples, epoch(1_000)).expect("valid recording");
        assert!(outcome.median_delta > STEADY_STATE_BUDGET);
        assert!(!outcome.pass);
    }

    #[test]
    fn one_fast_run_cannot_hide_a_systematic_regression() {
        // Four of five recordings show a 3% regression; one run is clean.
        let samples = samples_from_points([100.0; 5], [103.0, 103.0, 100.0, 103.0, 103.0]);
        let outcome = evaluate_runs(&samples, epoch(1_000)).expect("valid recording");
        assert!((outcome.median_delta - 0.03).abs() < 1e-12, "{}", outcome.median_delta);
        assert!(!outcome.pass);
    }

    #[test]
    fn min_side_delta_is_reported_but_does_not_decide() {
        // A single fast Kernox run drives the min-side delta under budget while
        // the median correctly fails the systematic regression.
        let samples = samples_from_points([100.0; 5], [100.0, 103.0, 103.0, 103.0, 103.0]);
        let outcome = evaluate_runs(&samples, epoch(1_000)).expect("valid recording");
        assert!((outcome.min_side_delta - 0.0).abs() < 1e-12);
        assert!((outcome.median_delta - 0.03).abs() < 1e-12);
        assert!(!outcome.pass);
    }

    #[test]
    fn fewer_than_five_recorded_runs_fail_in_evaluation() {
        let four = runs_with([0.01; 5]);
        let error = evaluate_runs(&four[..4], epoch(1_000)).expect_err("four runs are refused");
        assert!(error.contains("at least 5"), "{error}");
    }

    #[test]
    fn overlap_is_reported_both_ways() {
        let disjoint = runs_with([0.05, 0.10, 0.04, 0.06, 0.08]);
        let outcome = evaluate_runs(&disjoint, epoch(1_000)).expect("valid recording");
        assert!(!outcome.overlap);

        let mut overlapping = runs_with([0.03, 0.04, 0.05, 0.06, 0.07]);
        for sample in &mut overlapping {
            sample.kernox = Estimate { point: sample.kernox.point, lower: 100.5, upper: 102.0 };
        }
        let outcome = evaluate_runs(&overlapping, epoch(1_000)).expect("valid recording");
        assert!(outcome.overlap);

        let mut touching = runs_with([0.03, 0.04, 0.05, 0.06, 0.07]);
        for sample in &mut touching {
            sample.direct = Estimate { point: sample.direct.point, lower: 99.0, upper: 101.0 };
            sample.kernox = Estimate { point: sample.kernox.point, lower: 101.0, upper: 103.0 };
        }
        let outcome = evaluate_runs(&touching, epoch(1_000)).expect("valid recording");
        assert!(outcome.overlap, "closed intervals that touch still intersect");
    }

    #[test]
    fn overlap_does_not_waive_the_budget() {
        let mut samples = runs_with([0.10; 5]);
        for sample in &mut samples {
            sample.kernox = Estimate { point: sample.kernox.point, lower: 99.5, upper: 110.0 };
        }
        let outcome = evaluate_runs(&samples, epoch(1_000)).expect("valid recording");
        assert!(outcome.overlap);
        assert!(!outcome.pass);
    }

    #[test]
    fn stale_input_fails_closed_naming_the_path() {
        let samples = runs_with([0.01; 5]);
        let mut stale = samples.clone();
        stale[0].direct_modified = epoch(100);
        let error = evaluate_runs(&stale, epoch(1_000)).expect_err("stale direct fails");
        assert!(error.contains("run1/estimates.json"), "{error}");
        assert!(error.contains("stale"), "{error}");

        let mut stale = samples.clone();
        stale[4].kernox_modified = epoch(100);
        let error = evaluate_runs(&stale, epoch(1_000)).expect_err("stale kernox fails");
        assert!(error.contains("kernox-extracted-arc-dyn-trait/run5/estimates.json"), "{error}");
    }

    #[test]
    fn mixed_fresh_and_stale_sides_fail() {
        let mut samples = runs_with([0.01; 5]);
        // Fresh direct side, stale/reused Kernox side.
        samples[2].kernox_modified = epoch(1_050);
        let error = evaluate_runs(&samples, epoch(1_000)).expect_err("mixed pair fails");
        assert!(error.contains("not newer"), "{error}");

        // Stale direct side paired with a fresh Kernox side.
        let mut samples = runs_with([0.01; 5]);
        samples[2].direct_modified = epoch(1_050);
        let error = evaluate_runs(&samples, epoch(1_000)).expect_err("mixed pair fails");
        assert!(error.contains("not newer"), "{error}");
    }

    #[test]
    fn reordered_recordings_fail() {
        let mut samples = runs_with([0.01; 5]);
        // run2 direct written after run1 kernox? Move run2 direct before run1.
        samples[1].direct_modified = epoch(1_090);
        let error = evaluate_runs(&samples, epoch(1_000)).expect_err("out of order fails");
        assert!(error.contains("not recorded after run 1"), "{error}");

        // All files fresh, but run2 is newer than run3: not one ordered chain.
        let mut samples = runs_with([0.01; 5]);
        samples[2].direct_modified = epoch(1_150);
        samples[2].kernox_modified = epoch(1_160);
        let error = evaluate_runs(&samples, epoch(1_000)).expect_err("out of order fails");
        assert!(error.contains("not recorded after run 2"), "{error}");
    }

    #[test]
    fn no_recorded_runs_fail_closed() {
        let error = evaluate_runs(&[], epoch(1_000)).expect_err("empty recording fails");
        assert!(error.contains("no recorded benchmark runs"), "{error}");
    }

    #[test]
    fn gate_arguments_require_not_before_and_minimum_runs() {
        let missing = parse_gate_arguments(&[]).expect_err("requires --not-before");
        assert!(missing.contains("--not-before"), "{missing}");
        let unparsable = parse_gate_arguments(&["--not-before".to_owned(), "soon".to_owned()])
            .expect_err("epoch must parse");
        assert!(unparsable.contains("epoch"), "{unparsable}");
        let too_few = parse_gate_arguments(&[
            "--not-before".to_owned(),
            "1000".to_owned(),
            "--runs".to_owned(),
            "4".to_owned(),
        ])
        .expect_err("four runs are refused");
        assert!(too_few.contains(&MIN_RUNS.to_string()), "{too_few}");
        let unknown = parse_gate_arguments(&[
            "--not-before".to_owned(),
            "1000".to_owned(),
            "--other".to_owned(),
        ])
        .expect_err("unknown argument fails");
        assert!(unknown.contains("unknown"), "{unknown}");
    }

    #[test]
    fn gate_arguments_default_and_override() {
        let defaults = parse_gate_arguments(&["--not-before".to_owned(), "1000".to_owned()])
            .expect("defaults");
        assert_eq!(defaults.runs, DEFAULT_RUNS);
        assert_eq!(defaults.not_before, epoch(1_000));
        assert_eq!(defaults.criterion_dir, CriterionDir::Default);

        let overridden = parse_gate_arguments(&[
            "--criterion-dir".to_owned(),
            "/tmp/custom".to_owned(),
            "--runs".to_owned(),
            "6".to_owned(),
            "--not-before".to_owned(),
            "42".to_owned(),
        ])
        .expect("override");
        assert_eq!(overridden.criterion_dir, CriterionDir::Explicit(PathBuf::from("/tmp/custom")));
        assert_eq!(overridden.runs, 6);
        assert_eq!(overridden.not_before, epoch(42));
    }

    #[test]
    fn record_arguments_parse_and_validate() {
        let defaults = parse_record_arguments(&[]).expect("defaults");
        assert_eq!(defaults.runs, DEFAULT_RUNS);
        assert_eq!(defaults.criterion_dir, CriterionDir::Default);
        assert!(
            parse_record_arguments(&["--runs".to_owned(), "0".to_owned()]).is_err(),
            "zero runs are refused"
        );
        assert!(
            parse_record_arguments(&["--runs".to_owned(), "4".to_owned()]).is_err(),
            "four runs are refused"
        );
        assert!(
            parse_record_arguments(&["--criterion-dir".to_owned()]).is_err(),
            "missing value is refused"
        );
        assert!(parse_record_arguments(&["--other".to_owned()]).is_err());
    }

    #[test]
    fn explicit_criterion_dir_is_used_as_is() {
        assert_eq!(
            resolve_criterion_dir(&CriterionDir::Explicit(PathBuf::from("/tmp/custom")))
                .expect("explicit"),
            Path::new("/tmp/custom")
        );
    }

    #[test]
    fn default_criterion_dir_resolves_from_cargo_metadata_target() {
        let metadata =
            r#"{"target_directory":"/scratch/cargo-target/kernox--abc123","packages":[]}"#;
        let target = target_directory_from_metadata(metadata).expect("target directory");
        assert_eq!(target, Path::new("/scratch/cargo-target/kernox--abc123"));
        assert_eq!(
            target.join("criterion"),
            Path::new("/scratch/cargo-target/kernox--abc123/criterion")
        );
    }

    #[test]
    fn metadata_without_a_string_target_directory_fails_closed() {
        assert!(target_directory_from_metadata("{}").is_err());
        assert!(target_directory_from_metadata(r#"{"target_directory":7}"#).is_err());
        assert!(target_directory_from_metadata("not json").is_err());
    }
}
