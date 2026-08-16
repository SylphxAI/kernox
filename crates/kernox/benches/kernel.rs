//! Resolver scaling, indexed dependency lookup, and steady-state direct-handle benchmarks.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{hint::black_box, sync::Arc};

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use kernox::core::{CompositionSpec, GraphBuilder, GraphLimits};
use kernox::{
    AppBuilder, BoxFuture, Capability, CapabilityId, CapabilityOffer, CapabilityRequirement,
    InitializationContext, Plugin, PluginDescriptor, PluginError, PluginId, ProvisionSet,
};
use semver::{Version, VersionReq};

fn graph_resolution(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("graph-resolution");
    for size in [10_usize, 100, 1_000] {
        for density in [Density::Sparse, Density::Dense] {
            let spec = graph_spec(size, density);
            group.bench_with_input(
                BenchmarkId::new(density.name(), size),
                &spec,
                |bencher, spec| {
                    bencher.iter_batched(
                        || spec.clone(),
                        |input| black_box(GraphBuilder::from_spec(input).resolve().unwrap()),
                        BatchSize::SmallInput,
                    );
                },
            );
        }
    }
    group.finish();
}

#[derive(Clone, Copy)]
enum Density {
    Sparse,
    Dense,
}

impl Density {
    const fn name(self) -> &'static str {
        match self {
            Self::Sparse => "sparse",
            Self::Dense => "dense",
        }
    }
}

fn graph_spec(size: usize, density: Density) -> CompositionSpec {
    let mut plugins = Vec::with_capacity(size);
    for index in 0..size {
        let capability = CapabilityId::new(format!("dev.kernox.bench.c{index}")).unwrap();
        let mut descriptor = PluginDescriptor::new(
            PluginId::new(format!("dev.kernox.bench.p{index}")).unwrap(),
            Version::new(1, 0, 0),
        )
        .provide(CapabilityOffer::new(capability, Version::new(1, 0, 0)))
        .unwrap();
        let dependencies: Box<dyn Iterator<Item = usize>> = match density {
            Density::Sparse => Box::new(index.checked_sub(1).into_iter()),
            Density::Dense => Box::new(0..index),
        };
        for dependency in dependencies {
            descriptor = descriptor
                .require(CapabilityRequirement::exactly_one(
                    CapabilityId::new(format!("dev.kernox.bench.c{dependency}")).unwrap(),
                    VersionReq::parse("^1.0").unwrap(),
                ))
                .unwrap();
        }
        plugins.push(descriptor);
    }
    CompositionSpec {
        schema_version: 1,
        limits: GraphLimits {
            max_plugins: size,
            max_capabilities_per_plugin: size,
            max_edges: size.saturating_mul(size),
        },
        plugins,
        bindings: Vec::new(),
    }
}

fn indexed_requirement_lookup(criterion: &mut Criterion) {
    let consumer_id = PluginId::new("dev.kernox.bench.consumer").unwrap();
    let mut consumer = PluginDescriptor::new(consumer_id.clone(), Version::new(1, 0, 0));
    let mut builder = GraphBuilder::new();
    for index in 0..256_usize {
        let capability = CapabilityId::new(format!("dev.kernox.bench.requirement{index}")).unwrap();
        let provider = PluginDescriptor::new(
            PluginId::new(format!("dev.kernox.bench.provider{index}")).unwrap(),
            Version::new(1, 0, 0),
        )
        .provide(CapabilityOffer::new(capability.clone(), Version::new(1, 0, 0)))
        .unwrap();
        builder = builder.plugin(provider);
        consumer = consumer
            .require(CapabilityRequirement::exactly_one(
                capability,
                VersionReq::parse("^1.0").unwrap(),
            ))
            .unwrap();
    }
    let graph = builder.plugin(consumer).resolve().unwrap();
    let target = CapabilityId::new("dev.kernox.bench.requirement127").unwrap();
    let mut group = criterion.benchmark_group("indexed-requirement-lookup");
    group.bench_function("256-requirements", |bencher| {
        bencher.iter(|| black_box(graph.requirement(black_box(&consumer_id), black_box(&target))));
    });
    group.finish();
}

trait Compute: Send + Sync {
    fn apply(&self, value: u64) -> u64;
}

struct ComputeCapability;

impl Capability for ComputeCapability {
    type Interface = dyn Compute;

    const ID: &'static str = "dev.kernox.bench.compute";
    const VERSION: &'static str = "1.0.0";
}

struct NativeCompute;

impl Compute for NativeCompute {
    #[inline(never)]
    fn apply(&self, value: u64) -> u64 {
        value.rotate_left(13) ^ 0x9e37_79b9_7f4a_7c15
    }
}

struct ComputePlugin {
    descriptor: PluginDescriptor,
}

impl ComputePlugin {
    fn new() -> Self {
        let descriptor = PluginDescriptor::new(
            PluginId::new("dev.kernox.bench.compute-plugin").unwrap(),
            Version::new(1, 0, 0),
        )
        .provide(CapabilityOffer::new(
            CapabilityId::new(ComputeCapability::ID).unwrap(),
            Version::new(1, 0, 0),
        ))
        .unwrap();
        Self { descriptor }
    }
}

impl Plugin for ComputePlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let compute: Arc<dyn Compute> = Arc::new(NativeCompute);
        Box::pin(async move {
            ProvisionSet::new()
                .provide::<ComputeCapability>(compute)
                .map_err(|error| PluginError::new(error.tag(), error.to_string()))
        })
    }
}

fn steady_state_calls(criterion: &mut Criterion) {
    let direct: Arc<dyn Compute> = Arc::new(NativeCompute);
    let mut app = futures::executor::block_on(async {
        AppBuilder::new().plugin(ComputePlugin::new()).resolve().unwrap().start().await.unwrap()
    });
    let kernox = app
        .capability_from::<ComputeCapability>(
            &PluginId::new("dev.kernox.bench.compute-plugin").unwrap(),
        )
        .unwrap();
    let mut group = criterion.benchmark_group("steady-state-call");
    group.bench_function("direct-arc-dyn-trait", |bencher| {
        bencher.iter(|| black_box(direct.apply(black_box(41))));
    });
    group.bench_function("kernox-extracted-arc-dyn-trait", |bencher| {
        bencher.iter(|| black_box(kernox.apply(black_box(41))));
    });
    group.finish();
    let report = futures::executor::block_on(app.shutdown());
    assert!(report.is_clean());
}

criterion_group!(benches, graph_resolution, indexed_requirement_lookup, steady_state_calls);
criterion_main!(benches);
