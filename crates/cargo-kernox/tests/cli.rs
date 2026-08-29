//! Public `cargo kernox` command behavior.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

fn cargo_kernox() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-kernox"))
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/compositions").join(name)
}

#[test]
fn check_and_graph_accept_the_public_composition_fixtures() {
    let valid =
        cargo_kernox().args(["check", fixture("valid.json").to_str().unwrap()]).output().unwrap();
    assert!(valid.status.success(), "{}", String::from_utf8_lossy(&valid.stderr));
    assert_eq!(
        String::from_utf8(valid.stdout).unwrap().trim(),
        "valid: 2 plugin(s), 1 edge(s), 0 diagnostic(s), schema 1"
    );

    let verified = cargo_kernox()
        .args(["check", "--verified", fixture("verified.json").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(verified.status.success(), "{}", String::from_utf8_lossy(&verified.stderr));
    assert_eq!(
        String::from_utf8(verified.stdout).unwrap().trim(),
        "verified: 3 plugin(s), 3 source package(s), 2 edge(s), schema 1"
    );

    let graph = cargo_kernox()
        .args(["graph", fixture("valid.json").to_str().unwrap(), "--format", "dot"])
        .output()
        .unwrap();
    assert!(graph.status.success(), "{}", String::from_utf8_lossy(&graph.stderr));
    assert_eq!(
        String::from_utf8(graph.stdout).unwrap().trim_end(),
        "digraph kernox {\n  rankdir=LR;\n  \"dev.example.clock\";\n  \"dev.example.orders\";\n  \"dev.example.clock\" -> \"dev.example.orders\" [label=\"dev.example.clock\"];\n}"
    );
}

#[test]
fn verified_check_rejects_fewer_than_three_attributed_plugins() {
    let mut child = cargo_kernox()
        .args(["check", "--verified", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(
            br#"{
            "schema_version": 1,
            "limits": {"max_plugins": 10, "max_capabilities_per_plugin": 10, "max_edges": 10},
            "plugins": [
                {
                    "id": "dev.example.clock",
                    "version": "1.0.0",
                    "source": {"package": "pkg-clock", "repository": "https://example.invalid/clock"},
                    "provides": [{"id": "dev.example.clock", "version": "1.0.0"}],
                    "requires": [],
                    "conflicts": []
                },
                {
                    "id": "dev.example.orders",
                    "version": "1.0.0",
                    "source": {"package": "pkg-orders", "repository": "https://example.invalid/orders"},
                    "provides": [],
                    "requires": [{"id": "dev.example.clock", "version": "^1.0", "cardinality": "ExactlyOne"}],
                    "conflicts": []
                }
            ],
            "bindings": []
        }"#,
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr).unwrap().starts_with("conformance.too-few-plugins:"));
}

#[test]
fn empty_composition_renders_stable_dot() {
    let mut child = cargo_kernox()
        .args(["graph", "-", "--format", "dot"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(
            br#"{
            "schema_version": 1,
            "limits": {"max_plugins": 10, "max_capabilities_per_plugin": 10, "max_edges": 10},
            "plugins": [],
            "bindings": []
        }"#,
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim_end(),
        "digraph kernox {\n  rankdir=LR;\n}"
    );
}
