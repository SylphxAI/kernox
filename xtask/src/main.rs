//! Repository verification orchestration.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::Path,
    process::{Command, ExitCode},
};

const RELEASE_ORDER: &[&str] = &[
    "kernox-core",
    "kernox-runtime",
    "kernox-host-serverless",
    "kernox-host-tokio",
    "kernox-testkit",
    "kernox",
    "cargo-kernox",
];
const FORBIDDEN_HOSTED_LABELS: [&str; 3] = ["ubuntu-latest", "macos-latest", "windows-latest"];
const APPROVED_RUNNERS: [&str; 2] =
    ["sylphx-linux-standard", "[self-hosted, sylphx, macos, standard]"];

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next();
    let result = match command.as_deref() {
        Some("verify") if arguments.next().is_none() => verify(),
        Some("release-check") => {
            let arguments = arguments.collect::<Vec<_>>();
            release_check(&arguments)
        }
        _ => {
            eprintln!(
                "usage: cargo run -p xtask -- verify\n       cargo run -p xtask -- release-check [--version VERSION]"
            );
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask.failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn verify() -> Result<(), String> {
    println!("verify.workflow-runner-authority");
    enforce_workflow_runner_boundary()?;
    run("format", "cargo", &["fmt", "--all", "--", "--check"], &[])?;
    run(
        "check",
        "cargo",
        &["check", "--locked", "--workspace", "--all-targets", "--all-features"],
        &[],
    )?;
    run(
        "clippy",
        "cargo",
        &[
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
        &[],
    )?;
    run("tests", "cargo", &["test", "--locked", "--workspace", "--all-features"], &[])?;
    run(
        "docs",
        "cargo",
        &["doc", "--locked", "--workspace", "--all-features", "--no-deps"],
        &[("RUSTDOCFLAGS", "-D warnings")],
    )?;
    run(
        "minimal-core",
        "cargo",
        &["check", "--locked", "-p", "kernox-core", "--no-default-features"],
        &[],
    )?;
    enforce_core_dependency_boundary()?;
    run(
        "long-lived-example",
        "cargo",
        &["run", "--locked", "-p", "kernox-example-order-app", "--bin", "long_lived"],
        &[],
    )?;
    run(
        "serverless-example",
        "cargo",
        &["run", "--locked", "-p", "kernox-example-order-app", "--bin", "serverless"],
        &[],
    )?;
    run(
        "checkout-example",
        "cargo",
        &["run", "--locked", "-p", "kernox-example-checkout-app", "--bin", "checkout"],
        &[],
    )?;
    run(
        "worker-example",
        "cargo",
        &["run", "--locked", "-p", "kernox-example-worker-app", "--bin", "worker"],
        &[],
    )?;
    run(
        "clean-consumer",
        "cargo",
        &["run", "--locked", "--manifest-path", "fixtures/clean-consumer/Cargo.toml"],
        &[],
    )?;
    run(
        "clean-consumer-workload",
        "cargo",
        &[
            "run",
            "--release",
            "--locked",
            "--manifest-path",
            "fixtures/clean-consumer/Cargo.toml",
            "--",
            "--workload",
        ],
        &[],
    )?;
    run(
        "clean-consumer-serverless",
        "cargo",
        &[
            "run",
            "--locked",
            "--manifest-path",
            "fixtures/clean-consumer/Cargo.toml",
            "--",
            "--serverless",
        ],
        &[],
    )?;
    verify_clean_consumer_fanout()?;
    run("dependency-policy", "cargo", &["deny", "check"], &[])?;
    run("advisories", "cargo", &["audit", "--deny", "warnings"], &[])?;
    run("core-package", "cargo", &["package", "--locked", "-p", "kernox-core"], &[])?;
    Ok(())
}

fn verify_clean_consumer_fanout() -> Result<(), String> {
    run(
        "clean-consumer-fanout",
        "cargo",
        &[
            "run",
            "--locked",
            "--manifest-path",
            "fixtures/clean-consumer/Cargo.toml",
            "--",
            "--fanout",
        ],
        &[],
    )
}

fn release_check(arguments: &[String]) -> Result<(), String> {
    let expected_version = parse_release_version_argument(arguments)?;
    let metadata = cargo_metadata()?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| "cargo metadata packages missing".to_owned())?;
    let workspace_members = metadata["workspace_members"]
        .as_array()
        .ok_or_else(|| "cargo metadata workspace members missing".to_owned())?;
    let workspace_member_ids: BTreeSet<&str> =
        workspace_members.iter().filter_map(serde_json::Value::as_str).collect();
    let workspace_packages: BTreeMap<&str, &serde_json::Value> = packages
        .iter()
        .filter_map(|package| {
            let id = package["id"].as_str()?;
            workspace_member_ids.contains(id).then_some((package["name"].as_str()?, package))
        })
        .collect();

    let release_names: BTreeSet<&str> = RELEASE_ORDER.iter().copied().collect();
    let actual_publishable: BTreeSet<&str> = workspace_packages
        .iter()
        .filter_map(|(name, package)| is_crates_io_publishable(package).then_some(*name))
        .collect();
    if actual_publishable != release_names {
        return Err(format!(
            "publishable workspace packages do not match release order; expected {RELEASE_ORDER:?}, found {actual_publishable:?}"
        ));
    }

    let first_package = workspace_packages
        .get(RELEASE_ORDER[0])
        .ok_or_else(|| "release package kernox-core missing from workspace".to_owned())?;
    let version = first_package["version"]
        .as_str()
        .ok_or_else(|| "kernox-core version missing from cargo metadata".to_owned())?;
    if !version.starts_with("0.") {
        return Err(format!(
            "stable 1.x publication is disabled during development; release version is {version}"
        ));
    }
    if let Some(expected) = expected_version.as_deref() {
        if version != expected {
            return Err(format!("workspace version {version} does not match requested {expected}"));
        }
    }

    let order: BTreeMap<&str, usize> =
        RELEASE_ORDER.iter().enumerate().map(|(index, name)| (*name, index)).collect();
    for name in RELEASE_ORDER {
        let package = workspace_packages
            .get(name)
            .ok_or_else(|| format!("release package {name} missing from workspace"))?;
        let package_version = package["version"]
            .as_str()
            .ok_or_else(|| format!("{name} version missing from cargo metadata"))?;
        if package_version != version {
            return Err(format!(
                "release package {name} has version {package_version}, expected {version}"
            ));
        }
        for field in ["description", "license", "repository", "readme"] {
            let present = package[field].as_str().is_some_and(|value| !value.is_empty());
            if !present {
                return Err(format!(
                    "release package {name} is missing package metadata field {field}"
                ));
            }
        }
        let dependencies = package["dependencies"]
            .as_array()
            .ok_or_else(|| format!("{name} dependencies missing from cargo metadata"))?;
        for dependency in dependencies {
            let Some(dependency_name) = dependency["name"].as_str() else {
                continue;
            };
            let Some(dependency_index) = order.get(dependency_name) else {
                continue;
            };
            if dependency["path"].as_str().is_some() && *dependency_index >= order[name] {
                return Err(format!(
                    "release order places {name} before its path dependency {dependency_name}"
                ));
            }
        }
    }
    if !std::path::Path::new("Cargo.lock").is_file() {
        return Err("Cargo.lock is required for a reproducible release".to_owned());
    }
    println!("release.check version={version} packages={}", RELEASE_ORDER.join(","));
    Ok(())
}

fn parse_release_version_argument(arguments: &[String]) -> Result<Option<String>, String> {
    let mut expected = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--version" => {
                if expected.is_some() || index + 1 >= arguments.len() {
                    return Err("release-check expects one --version VERSION argument".to_owned());
                }
                expected = Some(arguments[index + 1].clone());
                index += 2;
            }
            argument => return Err(format!("unknown release-check argument {argument}")),
        }
    }
    Ok(expected)
}

fn cargo_metadata() -> Result<serde_json::Value, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--locked", "--format-version", "1"])
        .output()
        .map_err(|error| format!("could not inspect Cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err("cargo metadata failed".to_owned());
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid cargo metadata: {error}"))
}

fn is_crates_io_publishable(package: &serde_json::Value) -> bool {
    match &package["publish"] {
        serde_json::Value::Null => true,
        serde_json::Value::Array(registries) => {
            registries.iter().any(|registry| registry.as_str() == Some("crates-io"))
        }
        _ => false,
    }
}

fn enforce_workflow_runner_boundary() -> Result<(), String> {
    let workflow_dir = Path::new(".github/workflows");
    let mut workflow_paths = Vec::new();
    for entry in fs::read_dir(workflow_dir)
        .map_err(|error| format!("could not read {}: {error}", workflow_dir.display()))?
    {
        let path =
            entry.map_err(|error| format!("could not inspect workflow entry: {error}"))?.path();
        if matches!(path.extension().and_then(|extension| extension.to_str()), Some("yml" | "yaml"))
        {
            workflow_paths.push(path);
        }
    }
    workflow_paths.sort();
    if workflow_paths.is_empty() {
        return Err(format!("no workflow files found under {}", workflow_dir.display()));
    }

    for path in workflow_paths {
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        for label in FORBIDDEN_HOSTED_LABELS {
            if source.contains(label) {
                return Err(format!(
                    "{} contains forbidden GitHub-hosted runner label {label}",
                    path.display()
                ));
            }
        }

        let mut declarations = 0;
        for (index, line) in source.lines().enumerate() {
            let Some(value) = line.trim().strip_prefix("runs-on:") else {
                continue;
            };
            declarations += 1;
            let value = value.trim();
            if !APPROVED_RUNNERS.contains(&value) {
                return Err(format!(
                    "{}:{} declares unsupported runs-on {value:?}; use one static approved Sylphx profile",
                    path.display(),
                    index + 1
                ));
            }
        }
        if declarations == 0 {
            return Err(format!("{} declares no runner profile", path.display()));
        }
    }
    Ok(())
}

fn run(
    label: &str,
    program: &str,
    arguments: &[&str],
    environment: &[(&str, &str)],
) -> Result<(), String> {
    println!("verify.{label}");
    let mut command = Command::new(program);
    command.args(arguments);
    for (name, value) in environment {
        command.env(name, value);
    }
    let status = command.status().map_err(|error| format!("could not run {program}: {error}"))?;
    if status.success() { Ok(()) } else { Err(format!("{label} exited with {status}")) }
}

fn enforce_core_dependency_boundary() -> Result<(), String> {
    let metadata = cargo_metadata()?;
    let packages =
        metadata["packages"].as_array().ok_or_else(|| "metadata packages missing".to_owned())?;
    let names: BTreeMap<_, _> = packages
        .iter()
        .filter_map(|package| {
            Some((package["id"].as_str()?.to_owned(), package["name"].as_str()?.to_owned()))
        })
        .collect();
    let core = names
        .iter()
        .find_map(|(id, name)| (name == "kernox-core").then_some(id.clone()))
        .ok_or_else(|| "kernox-core missing from metadata".to_owned())?;
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or_else(|| "metadata resolve nodes missing".to_owned())?;
    let dependencies: BTreeMap<_, Vec<_>> = nodes
        .iter()
        .filter_map(|node| {
            Some((
                node["id"].as_str()?.to_owned(),
                node["dependencies"]
                    .as_array()?
                    .iter()
                    .filter_map(|dependency| dependency.as_str().map(str::to_owned))
                    .collect(),
            ))
        })
        .collect();
    let mut queue = VecDeque::from([core]);
    let mut visited = BTreeSet::new();
    while let Some(package) = queue.pop_front() {
        if !visited.insert(package.clone()) {
            continue;
        }
        if let Some(name) = names.get(&package) {
            if matches!(name.as_str(), "tokio" | "tokio-util") {
                return Err(format!(
                    "kernox-core transitively depends on forbidden runtime {name}"
                ));
            }
        }
        if let Some(children) = dependencies.get(&package) {
            queue.extend(children.iter().cloned());
        }
    }
    Ok(())
}
