//! Repository verification orchestration.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    let command = std::env::args().nth(1);
    if command.as_deref() != Some("verify") {
        eprintln!("usage: cargo run -p xtask -- verify");
        return ExitCode::from(2);
    }
    match verify() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("verify.failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn verify() -> Result<(), String> {
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
    run("dependency-policy", "cargo", &["deny", "check"], &[])?;
    run("advisories", "cargo", &["audit", "--deny", "warnings"], &[])?;
    run("core-package", "cargo", &["package", "--locked", "-p", "kernox-core"], &[])?;
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
    let output = Command::new("cargo")
        .args(["metadata", "--locked", "--format-version", "1"])
        .output()
        .map_err(|error| format!("could not inspect Cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err("cargo metadata failed".to_owned());
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid cargo metadata: {error}"))?;
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
