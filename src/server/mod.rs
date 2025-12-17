mod server;

use std::{net::ToSocketAddrs, sync::Arc};

use actflow::Engine;
use anyhow::{Result, anyhow};
use log::info;
use tonic::transport::server::Server as TonicServer;

use crate::proto::workflow_service_server::WorkflowServiceServer;
use server::WorkflowServer;

pub async fn start_server(
    engine: Arc<Engine>,
    addr: impl ToSocketAddrs,
    signal: impl Future<Output = ()>,
) -> Result<()> {
    let addr = addr.to_socket_addrs()?.next().ok_or_else(|| anyhow!("Invalid address"))?;
    info!("actflow server linstening on {}", addr);

    TonicServer::builder()
        .add_service(WorkflowServiceServer::new(WorkflowServer::new(engine)))
        .serve_with_shutdown(addr, signal)
        .await?;

    Ok(())
}
