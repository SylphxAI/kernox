//! `cargo kernox` bounded graph validation and inspection command.

use std::{
    fmt::{self, Write as _},
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand, ValueEnum};
use kernox_core::{CompositionSpec, GraphBuilder, GraphReport, ResolveError};
use thiserror::Error;

const MAX_SPEC_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "cargo kernox", bin_name = "cargo kernox")]
#[command(about = "Validate and inspect a Kernox capability graph")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate a composition and print a stable summary.
    Check {
        /// JSON composition file, or `-` for standard input.
        spec: PathBuf,
    },
    /// Resolve and render the selected capability graph.
    Graph {
        /// JSON composition file, or `-` for standard input.
        spec: PathBuf,
        /// Machine-readable JSON or Graphviz DOT.
        #[arg(long, value_enum, default_value_t = GraphFormat::Json)]
        format: GraphFormat,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum GraphFormat {
    Json,
    Dot,
}

#[derive(Debug, Error)]
enum CliError {
    #[error("failed to read composition: {0}")]
    Read(#[from] io::Error),
    #[error("composition exceeds {MAX_SPEC_BYTES} byte input bound")]
    InputTooLarge,
    #[error("invalid composition JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("composition rejected: {0}")]
    Resolve(#[from] ResolveError),
    #[error("failed to render graph: {0}")]
    Format(#[from] fmt::Error),
}

impl CliError {
    const fn tag(&self) -> &'static str {
        match self {
            Self::Read(_) => "cli.read-failed",
            Self::InputTooLarge => "cli.input-too-large",
            Self::Json(_) => "cli.invalid-json",
            Self::Resolve(error) => error.tag(),
            Self::Format(_) => "cli.format-failed",
        }
    }
}

fn main() -> ExitCode {
    let mut arguments: Vec<_> = std::env::args_os().collect();
    if arguments.get(1).is_some_and(|argument| argument == "kernox") {
        arguments.remove(1);
    }
    let cli = Cli::parse_from(arguments);
    match run(cli) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}: {error}", error.tag());
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<String, CliError> {
    match cli.command {
        Command::Check { spec } => {
            let report = load_and_resolve(&spec)?;
            Ok(format!(
                "valid: {} plugin(s), {} edge(s), schema {}",
                report.plugins.len(),
                report.edges.len(),
                report.schema_version
            ))
        }
        Command::Graph { spec, format } => {
            let report = load_and_resolve(&spec)?;
            match format {
                GraphFormat::Json => Ok(serde_json::to_string_pretty(&report)?),
                GraphFormat::Dot => Ok(render_dot(&report)?),
            }
        }
    }
}

fn load_and_resolve(path: &Path) -> Result<GraphReport, CliError> {
    let bytes = if path == Path::new("-") {
        read_bounded(io::stdin().lock())?
    } else {
        read_bounded(File::open(path)?)?
    };
    let spec: CompositionSpec = serde_json::from_slice(&bytes)?;
    Ok(GraphBuilder::from_spec(spec).resolve()?.report())
}

fn read_bounded(reader: impl Read) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    reader.take(MAX_SPEC_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SPEC_BYTES {
        return Err(CliError::InputTooLarge);
    }
    Ok(bytes)
}

fn render_dot(report: &GraphReport) -> Result<String, fmt::Error> {
    let mut output = String::from("digraph kernox {\n  rankdir=LR;\n");
    for plugin in &report.plugins {
        writeln!(&mut output, "  \"{}\";", plugin.id)?;
    }
    for edge in &report.edges {
        writeln!(
            &mut output,
            "  \"{}\" -> \"{}\" [label=\"{}\"];",
            edge.provider, edge.consumer, edge.capability
        )?;
    }
    output.push_str("}\n");
    Ok(output)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;

    #[test]
    fn accepts_bounded_empty_composition_and_renders_stable_dot() {
        let input = br#"{
            "schema_version": 1,
            "limits": {
                "max_plugins": 10,
                "max_capabilities_per_plugin": 10,
                "max_edges": 10
            },
            "plugins": [],
            "bindings": []
        }"#;
        let spec: CompositionSpec = serde_json::from_slice(input).unwrap();
        let report = GraphBuilder::from_spec(spec).resolve().unwrap().report();

        assert_eq!(render_dot(&report).unwrap(), "digraph kernox {\n  rankdir=LR;\n}\n");
    }

    #[test]
    fn rejects_oversized_input_before_json_parsing() {
        let reader = io::repeat(b'x').take(MAX_SPEC_BYTES + 1);
        let error = read_bounded(reader).unwrap_err();

        assert_eq!(error.tag(), "cli.input-too-large");
    }
}
