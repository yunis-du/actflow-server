use std::sync::Arc;

use actflow::EngineBuilder;
use anyhow::Result;
use log::info;
use tokio::{runtime::Runtime, signal::ctrl_c};

use crate::{common::shutdown::Shutdown, config::Config, logger::init_logger, server};

#[tokio::main]
pub async fn run(
    config: Config,
    runtime: Arc<Runtime>,
) -> Result<()> {
    // Init logger
    let logger = init_logger(&config.log)?;
    logger.start()?;

    info!("config {:#?}", config);

    info!("==================== Launching Actflow-Server ====================");

    // Build actflow engine
    let engine = EngineBuilder::new().runtime(runtime.clone()).build()?;
    engine.launch();

    let shutdown = Shutdown::new();

    let server_task = async { server::start_server(engine, format!("0.0.0.0:{}", config.server.port), shutdown.wait()).await };

    let sigint = ctrl_c();

    tokio::select! {
        res = server_task => res?,
        Ok(()) = sigint => (),
        else => return Ok(()),
    }
    shutdown.shutdown();
    info!("Gracefully shutting down.");

    Ok(())
}
