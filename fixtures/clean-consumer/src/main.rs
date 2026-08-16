//! A standalone typed application consuming Kernox from outside its workspace.

use std::{
    error::Error,
    sync::{Arc, Barrier},
    thread,
    time::Instant,
};

use futures::executor::block_on;
use kernox::core::PluginSource;
use kernox::{
    AppBuilder, BoxFuture, Capability, CapabilityId, CapabilityOffer, CapabilityRequirement,
    InitializationContext, Plugin, PluginDescriptor, PluginError, PluginId, ProvisionSet,
    ResolvedApp,
};
use kernox_testkit::verify_application;
use semver::{Version, VersionReq};

const CONTRACT_VERSION: &str = "1.0.0";
const CLOCK_PLUGIN_ID: &str = "dev.kernox.clean-consumer.clock";
const GREETING_PLUGIN_ID: &str = "dev.kernox.clean-consumer.greeting";
const APP_PLUGIN_ID: &str = "dev.kernox.clean-consumer.application";
const CLOCK_CAPABILITY_ID: &str = "dev.kernox.clean-consumer.clock";
const GREETING_CAPABILITY_ID: &str = "dev.kernox.clean-consumer.greeting";
const APP_CAPABILITY_ID: &str = "dev.kernox.clean-consumer.application";
const EXPECTED_MESSAGE: &str = "hello, Kernox @42ms";
const WORKLOAD_WORKERS: usize = 4;
const WORKLOAD_OPERATIONS_PER_WORKER: usize = 512;
const WORKLOAD_P99_BUDGET_NANOS: u64 = 5_000_000;
const WORKLOAD_MAX_BUDGET_NANOS: u64 = 100_000_000;

trait Clock: Send + Sync {
    fn now_millis(&self) -> u64;
}

struct ClockCapability;

impl Capability for ClockCapability {
    type Interface = dyn Clock;

    const ID: &'static str = CLOCK_CAPABILITY_ID;
    const VERSION: &'static str = CONTRACT_VERSION;
}

trait Greeting: Send + Sync {
    fn greet(&self, name: &str) -> String;
}

struct GreetingCapability;

impl Capability for GreetingCapability {
    type Interface = dyn Greeting;

    const ID: &'static str = GREETING_CAPABILITY_ID;
    const VERSION: &'static str = CONTRACT_VERSION;
}

trait Application: Send + Sync {
    fn greet(&self, name: &str) -> String;
}

struct ApplicationCapability;

impl Capability for ApplicationCapability {
    type Interface = dyn Application;

    const ID: &'static str = APP_CAPABILITY_ID;
    const VERSION: &'static str = CONTRACT_VERSION;
}

struct ClockPlugin {
    descriptor: PluginDescriptor,
}

impl ClockPlugin {
    fn new() -> Result<Self, Box<dyn Error>> {
        let descriptor = descriptor(CLOCK_PLUGIN_ID, "consumer-clock")?
            .provide(CapabilityOffer::new(capability_id(CLOCK_CAPABILITY_ID)?, version()?))?;
        Ok(Self { descriptor })
    }
}

impl Plugin for ClockPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let clock: Arc<dyn Clock> = Arc::new(FixedClock);
        Box::pin(async move {
            ProvisionSet::new().provide::<ClockCapability>(clock).map_err(provision_failure)
        })
    }
}

struct FixedClock;

impl Clock for FixedClock {
    fn now_millis(&self) -> u64 {
        42
    }
}

struct GreetingPlugin {
    descriptor: PluginDescriptor,
}

impl GreetingPlugin {
    fn new() -> Result<Self, Box<dyn Error>> {
        let descriptor = descriptor(GREETING_PLUGIN_ID, "consumer-greeting")?
            .provide(CapabilityOffer::new(capability_id(GREETING_CAPABILITY_ID)?, version()?))?
            .require(CapabilityRequirement::exactly_one(
                capability_id(CLOCK_CAPABILITY_ID)?,
                VersionReq::parse("^1.0")?,
            ))?;
        Ok(Self { descriptor })
    }
}

impl Plugin for GreetingPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let clock = context.require::<ClockCapability>();
        Box::pin(async move {
            let greeting: Arc<dyn Greeting> =
                Arc::new(GreetingService { clock: clock.map_err(access_failure)? });
            ProvisionSet::new().provide::<GreetingCapability>(greeting).map_err(provision_failure)
        })
    }
}

struct GreetingService {
    clock: Arc<dyn Clock>,
}

impl Greeting for GreetingService {
    fn greet(&self, name: &str) -> String {
        format!("hello, {name} @{}ms", self.clock.now_millis())
    }
}

struct ApplicationPlugin {
    descriptor: PluginDescriptor,
}

impl ApplicationPlugin {
    fn new() -> Result<Self, Box<dyn Error>> {
        let descriptor = descriptor(APP_PLUGIN_ID, "consumer-application")?
            .provide(CapabilityOffer::new(capability_id(APP_CAPABILITY_ID)?, version()?))?
            .require(CapabilityRequirement::exactly_one(
                capability_id(GREETING_CAPABILITY_ID)?,
                VersionReq::parse("^1.0")?,
            ))?;
        Ok(Self { descriptor })
    }
}

impl Plugin for ApplicationPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        let greeting = context.require::<GreetingCapability>();
        Box::pin(async move {
            let application: Arc<dyn Application> =
                Arc::new(ApplicationService { greeting: greeting.map_err(access_failure)? });
            ProvisionSet::new()
                .provide::<ApplicationCapability>(application)
                .map_err(provision_failure)
        })
    }
}

struct ApplicationService {
    greeting: Arc<dyn Greeting>,
}

impl Application for ApplicationService {
    fn greet(&self, name: &str) -> String {
        self.greeting.greet(name)
    }
}

fn compose() -> Result<ResolvedApp, Box<dyn Error>> {
    AppBuilder::new()
        .plugin(ApplicationPlugin::new()?)
        .plugin(GreetingPlugin::new()?)
        .plugin(ClockPlugin::new()?)
        .resolve()
        .map_err(Into::into)
}

fn descriptor(id: &str, package: &str) -> Result<PluginDescriptor, Box<dyn Error>> {
    Ok(PluginDescriptor::new(PluginId::new(id)?, version()?).sourced_from(source(package)?))
}

fn source(package: &str) -> Result<PluginSource, Box<dyn Error>> {
    PluginSource::new(
        package,
        Some("https://github.com/SylphxAI/kernox/tree/main/fixtures/clean-consumer".to_owned()),
    )
    .map_err(Into::into)
}

fn capability_id(value: &str) -> Result<CapabilityId, Box<dyn Error>> {
    CapabilityId::new(value).map_err(Into::into)
}

fn version() -> Result<Version, Box<dyn Error>> {
    Version::parse(CONTRACT_VERSION).map_err(Into::into)
}

fn access_failure(error: kernox::runtime::AccessError) -> PluginError {
    let tag = error.tag();
    let message = error.to_string();
    drop(error);
    PluginError::new(tag, message)
}

fn provision_failure(error: kernox::runtime::ProvisionError) -> PluginError {
    let tag = error.tag();
    let message = error.to_string();
    drop(error);
    PluginError::new(tag, message)
}

async fn run_smoke() -> Result<(), Box<dyn Error>> {
    let mut running = compose()?.start().await?;
    let application =
        running.capability_from::<ApplicationCapability>(&PluginId::new(APP_PLUGIN_ID)?)?;
    let message = application.greet("Kernox");
    drop(application);
    let shutdown = running.shutdown().await;
    if !shutdown.is_clean() {
        return Err(
            format!("typed consumer shutdown had {} failure(s)", shutdown.failures.len()).into()
        );
    }

    let report = verify_application(compose()?).await?;

    if message != EXPECTED_MESSAGE {
        return Err(format!("unexpected typed application result: {message}").into());
    }
    println!("clean consumer direct call: {message}");
    println!("clean consumer verified {} plugins", report.plugin_count);
    Ok(())
}

async fn run_workload() -> Result<(), Box<dyn Error>> {
    let mut running = compose()?.start().await?;
    let application =
        running.capability_from::<ApplicationCapability>(&PluginId::new(APP_PLUGIN_ID)?)?;
    let measurement = measure_workload(&application);
    drop(application);

    let shutdown = running.shutdown().await;
    if !shutdown.is_clean() {
        return Err(format!(
            "typed consumer workload shutdown had {} failure(s)",
            shutdown.failures.len()
        )
        .into());
    }
    let samples = measurement?;

    let mut ordered = samples;
    ordered.sort_unstable();
    let p50 = percentile(&ordered, 50);
    let p95 = percentile(&ordered, 95);
    let p99 = percentile(&ordered, 99);
    let max = *ordered.last().ok_or("workload produced no samples")?;
    if p99 > WORKLOAD_P99_BUDGET_NANOS {
        return Err(format!(
            "typed consumer workload p99 {p99}ns exceeded {WORKLOAD_P99_BUDGET_NANOS}ns budget"
        )
        .into());
    }
    if max > WORKLOAD_MAX_BUDGET_NANOS {
        return Err(format!(
            "typed consumer workload max {max}ns exceeded {WORKLOAD_MAX_BUDGET_NANOS}ns budget"
        )
        .into());
    }
    println!(
        "clean consumer workload: workers={WORKLOAD_WORKERS} operations={} p50={p50}ns p95={p95}ns p99={p99}ns max={max}ns",
        ordered.len()
    );
    Ok(())
}

fn measure_workload(application: &Arc<dyn Application>) -> Result<Vec<u64>, Box<dyn Error>> {
    let samples = thread::scope(|scope| {
        let start_barrier = Arc::new(Barrier::new(WORKLOAD_WORKERS));
        let handles = (0..WORKLOAD_WORKERS)
            .map(|_| {
                let application = Arc::clone(application);
                let start_barrier = Arc::clone(&start_barrier);
                scope.spawn(move || {
                    start_barrier.wait();
                    let mut samples = Vec::with_capacity(WORKLOAD_OPERATIONS_PER_WORKER);
                    for _ in 0..WORKLOAD_OPERATIONS_PER_WORKER {
                        let started = Instant::now();
                        let message = application.greet("Kernox");
                        let elapsed = u64::try_from(started.elapsed().as_nanos())
                            .map_err(|_| "workload sample duration overflow".to_owned())?;
                        if message != EXPECTED_MESSAGE {
                            return Err(format!("unexpected workload result: {message}"));
                        }
                        samples.push(elapsed);
                    }
                    Ok::<_, String>(samples)
                })
            })
            .collect::<Vec<_>>();

        let mut samples = Vec::with_capacity(WORKLOAD_WORKERS * WORKLOAD_OPERATIONS_PER_WORKER);
        for handle in handles {
            let worker_samples =
                handle.join().map_err(|_| "workload worker panicked".to_owned())??;
            samples.extend(worker_samples);
        }
        Ok::<_, String>(samples)
    })
    .map_err(|error| -> Box<dyn Error> { error.into() })?;
    Ok(samples)
}

fn percentile(samples: &[u64], percentile: usize) -> u64 {
    let index = ((samples.len() - 1) * percentile).div_ceil(100);
    samples[index]
}

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::args().nth(1).as_deref() == Some("--workload") {
        block_on(run_workload())
    } else {
        block_on(run_smoke())
    }
}
