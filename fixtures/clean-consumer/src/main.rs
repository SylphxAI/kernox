//! A standalone application consuming Kernox from outside its workspace.

use std::error::Error;

use futures::executor::block_on;
use kernox::core::PluginSource;
use kernox::{
    AppBuilder, BoxFuture, InitializationContext, Plugin, PluginDescriptor, PluginError, PluginId,
    ProvisionSet, ResolvedApp,
};
use kernox_testkit::verify_application;
use semver::Version;

struct ConsumerPlugin {
    descriptor: PluginDescriptor,
}

impl ConsumerPlugin {
    fn new(id: &str, package: &str) -> Result<Self, Box<dyn Error>> {
        let source = PluginSource::new(
            package,
            Some("https://github.com/SylphxAI/kernox/tree/main/fixtures/clean-consumer".to_owned()),
        )?;
        let descriptor =
            PluginDescriptor::new(PluginId::new(id)?, Version::new(1, 0, 0)).sourced_from(source);
        Ok(Self { descriptor })
    }
}

impl Plugin for ConsumerPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn initialize<'a>(
        &'a mut self,
        _context: InitializationContext<'a>,
    ) -> BoxFuture<'a, Result<ProvisionSet, PluginError>> {
        Box::pin(async { Ok(ProvisionSet::new()) })
    }
}

fn compose() -> Result<ResolvedApp, Box<dyn Error>> {
    AppBuilder::new()
        .plugin(ConsumerPlugin::new("dev.kernox.consumer.clock", "consumer-clock")?)
        .plugin(ConsumerPlugin::new("dev.kernox.consumer.store", "consumer-store")?)
        .plugin(ConsumerPlugin::new("dev.kernox.consumer.service", "consumer-service")?)
        .resolve()
        .map_err(Into::into)
}

fn main() -> Result<(), Box<dyn Error>> {
    let report = block_on(verify_application(compose()?))?;
    println!("clean consumer verified {} plugins", report.plugin_count);
    Ok(())
}
