//! Fail-closed steady-state budget gate over Criterion `estimates.json` output.
//!
//! The gate compares the `mean.point_estimate` of the recorded steady-state
//! dynamic-dispatch benchmark against the direct-handle control:
//!
//! ```text
//! delta = (kernox_point - direct_point) / direct_point
//! ```
//!
//! The declared budget is 2%: a delta of exactly `0.02` passes because the
//! acceptance claim is "no greater than 2%", and any delta above it fails.
//! The point estimate decides; the two `mean.confidence_interval` ranges are
//! reported as overlapping or not, and that overlap never waives the budget.
//! Missing files, absent benchmarks, unparsable JSON, and missing/non-finite
//! or non-positive fields fail closed.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Declared acceptance budget for the steady-state point-estimate delta.
pub(crate) const STEADY_STATE_BUDGET: f64 = 0.02;

/// Criterion benchmark path of the direct `Arc<dyn Trait>` control call.
pub(crate) const DIRECT_BENCHMARK: &str = "steady-state-call/direct-arc-dyn-trait";

/// Criterion benchmark path of the Kernox-extracted `Arc<dyn Trait>` call.
pub(crate) const KERNOX_BENCHMARK: &str = "steady-state-call/kernox-extracted-arc-dyn-trait";

const ESTIMATES_FILE: &str = "estimates.json";
const NEW_SAMPLES_DIR: &str = "new";

/// A Criterion mean estimate and its confidence interval, in nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Estimate {
    pub(crate) point: f64,
    pub(crate) lower: f64,
    pub(crate) upper: f64,
}

/// The budget decision: the measured delta, whether the two confidence
/// intervals intersect, and whether the point-estimate budget holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BudgetOutcome {
    pub(crate) delta: f64,
    pub(crate) overlap: bool,
    pub(crate) pass: bool,
}

impl Estimate {
    /// Builds a validated estimate; every value must be finite, bounds must be
    /// ordered, and the point estimate must be positive for a defined delta.
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
        if lower > upper {
            return Err("field mean.confidence_interval.lower_bound exceeds upper_bound".to_owned());
        }
        if point <= 0.0 {
            return Err("field mean.point_estimate is not positive".to_owned());
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

/// Applies the declared rule: `delta <= 0.02` passes, anything above fails.
/// The point estimates decide; interval overlap is reported but never waives
/// the budget.
pub(crate) fn evaluate(direct: Estimate, kernox: Estimate) -> Result<BudgetOutcome, String> {
    let delta = (kernox.point - direct.point) / direct.point;
    if !delta.is_finite() {
        return Err(format!(
            "computed delta is not finite (direct point estimate {})",
            direct.point
        ));
    }
    // Closed-interval intersection: touching endpoints still count as overlap.
    let overlap = direct.lower <= kernox.upper && kernox.lower <= direct.upper;
    Ok(BudgetOutcome { delta, overlap, pass: delta <= STEADY_STATE_BUDGET })
}

/// Location of the recorded estimates for one Criterion benchmark path.
pub(crate) fn estimate_path(criterion_dir: &Path, benchmark: &str) -> PathBuf {
    criterion_dir.join(benchmark).join(NEW_SAMPLES_DIR).join(ESTIMATES_FILE)
}

fn read_estimate(path: &Path) -> Result<Estimate, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    parse_estimate(&source, path)
}

/// Runs the gate against `criterion_dir`, printing machine-readable lines and
/// failing (non-zero exit) when the budget is exceeded or evidence is missing.
pub(crate) fn run(criterion_dir: &Path) -> Result<(), String> {
    let direct = read_estimate(&estimate_path(criterion_dir, DIRECT_BENCHMARK))?;
    let kernox = read_estimate(&estimate_path(criterion_dir, KERNOX_BENCHMARK))?;
    let outcome = evaluate(direct, kernox)?;
    println!("bench-budget.criterion-dir={}", criterion_dir.display());
    println!("bench-budget.direct={:.6}", direct.point);
    println!("bench-budget.kernox={:.6}", kernox.point);
    println!("bench-budget.direct.interval=[{:.6}, {:.6}]", direct.lower, direct.upper);
    println!("bench-budget.kernox.interval=[{:.6}, {:.6}]", kernox.lower, kernox.upper);
    println!("bench-budget.delta={:.6}", outcome.delta);
    println!("bench-budget.overlap={}", outcome.overlap);
    println!("bench-budget.result={}", if outcome.pass { "pass" } else { "fail" });
    if outcome.pass {
        Ok(())
    } else {
        Err(format!(
            "steady-state point-estimate delta {:.6} exceeds declared budget {:.6}",
            outcome.delta, STEADY_STATE_BUDGET
        ))
    }
}

/// Criterion directory selection: the resolved Cargo target directory by
/// default, or an explicit `--criterion-dir` override.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CriterionDir {
    Default,
    Explicit(PathBuf),
}

/// Parses `bench-budget [--criterion-dir <dir>]`; without the override the
/// directory is resolved from Cargo's actual target directory.
pub(crate) fn parse_criterion_dir(arguments: &[String]) -> Result<CriterionDir, String> {
    let mut selection = CriterionDir::Default;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--criterion-dir" => {
                if index + 1 >= arguments.len() {
                    return Err("bench-budget expects --criterion-dir DIR".to_owned());
                }
                selection = CriterionDir::Explicit(PathBuf::from(&arguments[index + 1]));
                index += 2;
            }
            argument => return Err(format!("unknown bench-budget argument {argument}")),
        }
    }
    Ok(selection)
}

/// Resolves the selection: an explicit directory is used as-is; otherwise the
/// Cargo target directory is read from `cargo metadata`, so a configured
/// `CARGO_TARGET_DIR` (or any other target-directory configuration) is honored
/// instead of assuming a relative `target` directory.
pub(crate) fn resolve_criterion_dir(selection: CriterionDir) -> Result<PathBuf, String> {
    match selection {
        CriterionDir::Explicit(criterion_dir) => Ok(criterion_dir),
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use std::path::{Path, PathBuf};

    use super::{
        CriterionDir, DIRECT_BENCHMARK, Estimate, KERNOX_BENCHMARK, STEADY_STATE_BUDGET,
        estimate_path, evaluate, parse_criterion_dir, parse_estimate, resolve_criterion_dir,
        target_directory_from_metadata,
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

    fn estimate(point: f64, lower: f64, upper: f64) -> Estimate {
        Estimate { point, lower, upper }
    }

    #[test]
    fn parses_the_observed_estimates_schema() {
        let parsed = parse_estimate(DIRECT_SOURCE, Path::new("direct/estimates.json"))
            .expect("observed Criterion schema parses");
        assert_eq!(parsed, estimate(100.0, 99.0, 101.0));
    }

    #[test]
    fn delta_exactly_two_percent_passes() {
        let outcome = evaluate(estimate(100.0, 99.0, 101.0), estimate(102.0, 101.5, 102.5))
            .expect("definition is valid");
        assert!((outcome.delta - STEADY_STATE_BUDGET).abs() < f64::EPSILON);
        assert!(outcome.pass);
    }

    #[test]
    fn delta_above_two_percent_fails() {
        let above = evaluate(estimate(100.0, 99.0, 101.0), estimate(102.01, 101.5, 102.5))
            .expect("definition is valid");
        assert!(above.delta > STEADY_STATE_BUDGET);
        assert!(!above.pass);

        let larger = evaluate(estimate(100.0, 99.0, 101.0), estimate(103.0, 102.0, 104.0))
            .expect("definition is valid");
        assert!(!larger.pass);
    }

    #[test]
    fn interval_overlap_is_reported_both_ways() {
        let overlapping =
            evaluate(estimate(100.0, 99.0, 101.0), estimate(100.5, 100.0, 101.5)).expect("valid");
        assert!(overlapping.overlap);
        assert!(overlapping.pass);

        let disjoint =
            evaluate(estimate(100.0, 99.0, 100.5), estimate(103.0, 102.5, 103.5)).expect("valid");
        assert!(!disjoint.overlap);

        let touching =
            evaluate(estimate(100.0, 99.0, 101.0), estimate(102.0, 101.0, 103.0)).expect("valid");
        assert!(touching.overlap, "closed intervals that touch still intersect");
    }

    #[test]
    fn overlap_does_not_waive_the_budget() {
        let outcome = evaluate(estimate(100.0, 99.0, 101.0), estimate(103.0, 100.5, 103.5))
            .expect("definition is valid");
        assert!(outcome.overlap);
        assert!(!outcome.pass);
    }

    #[test]
    fn missing_mean_field_fails_closed() {
        let error = parse_estimate("{}", Path::new("x/estimates.json")).expect_err("must fail");
        assert!(error.contains("missing field mean"), "{error}");
    }

    #[test]
    fn missing_interval_and_point_fields_name_the_field() {
        let missing_point =
            r#"{"mean":{"confidence_interval":{"lower_bound":1.0,"upper_bound":2.0}}}"#;
        let error =
            parse_estimate(missing_point, Path::new("x/estimates.json")).expect_err("must fail");
        assert!(error.contains("point_estimate"), "{error}");

        let missing_lower =
            r#"{"mean":{"confidence_interval":{"upper_bound":2.0},"point_estimate":1.0}}"#;
        let error =
            parse_estimate(missing_lower, Path::new("x/estimates.json")).expect_err("must fail");
        assert!(error.contains("lower_bound"), "{error}");
    }

    #[test]
    fn unparsable_json_fails_closed_and_names_the_path() {
        let error = parse_estimate("not json", Path::new("x/estimates.json")).expect_err("fails");
        assert!(error.contains("x/estimates.json"), "{error}");
        assert!(error.contains("unparsable"), "{error}");
    }

    #[test]
    fn non_finite_values_fail_closed() {
        // serde_json rejects out-of-range numeric literals before any value is
        // constructed, so an overflowing estimate is an unparsable document.
        let overflow = r#"{"mean":{"confidence_interval":{"lower_bound":1.0,"upper_bound":2.0},"point_estimate":1e400}}"#;
        let error = parse_estimate(overflow, Path::new("x/estimates.json")).expect_err("fails");
        assert!(error.contains("x/estimates.json"), "{error}");

        let nan = Estimate::new(f64::NAN, 1.0, 2.0).expect_err("NaN fails");
        assert!(nan.contains("point_estimate"), "{nan}");
        let nan_bound = Estimate::new(1.0, f64::NAN, 2.0).expect_err("NaN bound fails");
        assert!(nan_bound.contains("lower_bound"), "{nan_bound}");
        let infinite_bound = Estimate::new(1.0, 0.5, f64::INFINITY).expect_err("inf fails");
        assert!(infinite_bound.contains("upper_bound"), "{infinite_bound}");
    }

    #[test]
    fn non_positive_direct_point_estimate_fails_closed() {
        let error = Estimate::new(0.0, 0.0, 1.0).expect_err("zero is not a usable divisor");
        assert!(error.contains("not positive"), "{error}");
    }

    #[test]
    fn estimate_paths_match_the_observed_criterion_layout() {
        assert_eq!(
            estimate_path(Path::new("target/criterion"), DIRECT_BENCHMARK),
            Path::new("target/criterion/steady-state-call/direct-arc-dyn-trait/new/estimates.json")
        );
        assert_eq!(
            estimate_path(Path::new("target/criterion"), KERNOX_BENCHMARK),
            Path::new(
                "target/criterion/steady-state-call/kernox-extracted-arc-dyn-trait/new/estimates.json"
            )
        );
    }

    #[test]
    fn criterion_dir_argument_defaults_and_overrides() {
        assert_eq!(parse_criterion_dir(&[]).expect("default"), CriterionDir::Default);
        assert_eq!(
            parse_criterion_dir(&["--criterion-dir".to_owned(), "/tmp/custom".to_owned()])
                .expect("override"),
            CriterionDir::Explicit(PathBuf::from("/tmp/custom"))
        );
        assert!(parse_criterion_dir(&["--criterion-dir".to_owned()]).is_err());
        assert!(parse_criterion_dir(&["--other".to_owned()]).is_err());
    }

    #[test]
    fn explicit_criterion_dir_is_used_as_is() {
        assert_eq!(
            resolve_criterion_dir(CriterionDir::Explicit(PathBuf::from("/tmp/custom")))
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
