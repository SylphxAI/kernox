//! Graph-level North Star attribution checks shared by the testkit and CLI.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::{PluginId, ResolvedGraph};

/// Minimum number of separately attributed plugins required by the verified
/// application contract.
pub const MINIMUM_VERIFIED_PLUGINS: usize = 3;

/// Graph-only evidence that an application composition is source-attributed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttributionReport {
    /// Number of plugins in the resolved graph.
    pub plugin_count: usize,
    /// Unique source package names in stable plugin identity order.
    pub source_packages: Vec<String>,
}

/// Failure to meet the verified-application graph attribution contract.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AttributionError {
    /// The application is too small to exercise the North Star composition path.
    #[error("application has {actual} plugin(s); at least {minimum} are required")]
    TooFewPlugins {
        /// Number of plugins in the graph.
        actual: usize,
        /// Minimum accepted plugin count.
        minimum: usize,
    },
    /// A plugin cannot be tied to an independently attributable source package.
    #[error("plugin {plugin} is missing complete source attribution")]
    MissingSource {
        /// Plugin without a package and repository attribution.
        plugin: PluginId,
    },
    /// Two plugins claim the same source package within one application.
    #[error("source package {package:?} is used by more than one plugin")]
    DuplicateSourcePackage {
        /// Repeated source package name.
        package: String,
    },
}

impl AttributionError {
    /// Returns the stable machine-readable failure tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::TooFewPlugins { .. } => "conformance.too-few-plugins",
            Self::MissingSource { .. } => "conformance.missing-source",
            Self::DuplicateSourcePackage { .. } => "conformance.duplicate-source-package",
        }
    }
}

/// Checks that a resolved graph meets the North Star source-attribution shape.
///
/// This does not start plugins, publish packages, or prove legal ownership.
///
/// # Errors
///
/// Returns [`AttributionError`] when the graph has fewer than
/// [`MINIMUM_VERIFIED_PLUGINS`], a plugin lacks package and repository
/// attribution, or two plugins share a source package name.
pub fn verify_graph_attribution(
    graph: &ResolvedGraph,
) -> Result<AttributionReport, AttributionError> {
    let plugin_count = graph.plugins().count();
    if plugin_count < MINIMUM_VERIFIED_PLUGINS {
        return Err(AttributionError::TooFewPlugins {
            actual: plugin_count,
            minimum: MINIMUM_VERIFIED_PLUGINS,
        });
    }

    let mut packages = BTreeSet::new();
    let mut source_packages = Vec::with_capacity(plugin_count);
    for descriptor in graph.plugins() {
        let Some(source) = descriptor.source() else {
            return Err(AttributionError::MissingSource { plugin: descriptor.id().clone() });
        };
        if source.repository().is_none() {
            return Err(AttributionError::MissingSource { plugin: descriptor.id().clone() });
        }
        let package = source.package().to_owned();
        if !packages.insert(package.clone()) {
            return Err(AttributionError::DuplicateSourcePackage { package });
        }
        source_packages.push(package);
    }

    Ok(AttributionReport { plugin_count, source_packages })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use semver::Version;

    use super::*;
    use crate::{GraphBuilder, PluginDescriptor, PluginSource};

    fn plugin(id: &str, package: Option<&str>) -> PluginDescriptor {
        let descriptor = PluginDescriptor::new(PluginId::new(id).unwrap(), Version::new(1, 0, 0));
        match package {
            Some(package) => descriptor.sourced_from(
                PluginSource::new(package, Some("https://example.invalid/repo".to_owned()))
                    .unwrap(),
            ),
            None => descriptor,
        }
    }

    #[test]
    fn accepts_three_unique_source_packages() {
        let graph = GraphBuilder::new()
            .plugin(plugin("dev.example.alpha", Some("pkg-alpha")))
            .plugin(plugin("dev.example.beta", Some("pkg-beta")))
            .plugin(plugin("dev.example.gamma", Some("pkg-gamma")))
            .resolve()
            .unwrap();

        let report = verify_graph_attribution(&graph).unwrap();
        assert_eq!(report.plugin_count, 3);
        assert_eq!(report.source_packages, ["pkg-alpha", "pkg-beta", "pkg-gamma"]);
    }

    #[test]
    fn rejects_too_few_missing_and_duplicate_attribution() {
        let too_few = GraphBuilder::new()
            .plugin(plugin("dev.example.alpha", Some("pkg-alpha")))
            .resolve()
            .unwrap();
        assert_eq!(
            verify_graph_attribution(&too_few).unwrap_err().tag(),
            "conformance.too-few-plugins"
        );

        let missing = GraphBuilder::new()
            .plugin(plugin("dev.example.alpha", Some("pkg-alpha")))
            .plugin(plugin("dev.example.beta", None))
            .plugin(plugin("dev.example.gamma", Some("pkg-gamma")))
            .resolve()
            .unwrap();
        assert_eq!(
            verify_graph_attribution(&missing).unwrap_err().tag(),
            "conformance.missing-source"
        );

        let no_repo = PluginDescriptor::new(
            PluginId::new("dev.example.beta").unwrap(),
            Version::new(1, 0, 0),
        )
        .sourced_from(PluginSource::new("pkg-beta", None).unwrap());
        let missing_repo = GraphBuilder::new()
            .plugin(plugin("dev.example.alpha", Some("pkg-alpha")))
            .plugin(no_repo)
            .plugin(plugin("dev.example.gamma", Some("pkg-gamma")))
            .resolve()
            .unwrap();
        assert_eq!(
            verify_graph_attribution(&missing_repo).unwrap_err().tag(),
            "conformance.missing-source"
        );

        let duplicate = GraphBuilder::new()
            .plugin(plugin("dev.example.alpha", Some("pkg-shared")))
            .plugin(plugin("dev.example.beta", Some("pkg-shared")))
            .plugin(plugin("dev.example.gamma", Some("pkg-gamma")))
            .resolve()
            .unwrap();
        assert_eq!(
            verify_graph_attribution(&duplicate).unwrap_err().tag(),
            "conformance.duplicate-source-package"
        );
    }
}
