use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tracing::Instrument;

use crate::{any_ok, AppContext, transfer};
use crate::config::AppConfig;
use crate::conn;
use crate::transfer::TransferResult;

struct PortHandler<IO> {
    context: Arc<AppContext>,
    app_config: Arc<AppConfig>,
    addr: SocketAddr,
    src: IO,
}

impl PortHandler<TcpStream> {
    #[rustfmt::skip]
    pub async fn run(mut self) -> Result<()> {
        let span = tracing::info_span!("tunnel", from = self.addr.to_string(), to = self.app_config.name());
        async move {
            let cli = &self.context.cli;

            loop {
                tracing::debug!("connect");
                let dst_connect_fut = timeout(cli.target_connect_timeout(), conn::connect(&self.app_config));
                let mut dst = any_ok!(any_ok!(dst_connect_fut.await));

                tracing::debug!("transferring...");
                let transfer_fut = transfer::transfer(
                    &mut self.src,
                    &mut dst,
                    cli.inactivity_timeout()
                );

                match transfer_fut.await {
                    TransferResult::SourceErr(e) => return Err(e.into()),
                    TransferResult::SourceEOF => anyhow::bail!("source connection EOF"),
                    TransferResult::TargetErr(e) => tracing::debug!("target connection error, reason: {:?}", e),
                    TransferResult::TargetEOF => tracing::debug!("target connection EOF"),
                }
            }
        }
            .instrument(span)
            .await
    }
}

async fn start_port_listener(context: Arc<AppContext>, app: AppConfig, port: u16) -> Result<()> {
    tracing::info!("starting listener for app: {} on port: {}", app.name(), port);

    let app_ref = Arc::new(app);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await
        .with_context(|| format!("failed to listen on port {}", port))?;
    loop {
        let (io, addr) = match listener.accept().await {
            Err(e) => {
                tracing::error!("failed to establish connection on port: {}, reason: {:?}", port, e);
                continue;
            }
            Ok(res) => res,
        };
        tracing::info!("established connection on port: {} from addr: {:?}", port, addr);

        let port_handler = PortHandler {
            context: context.clone(),
            app_config: app_ref.clone(),
            addr,
            src: io,
        };
        tokio::spawn(async move {
            if let Err(e) = port_handler.run().await {
                tracing::info!("aborted connection on port: {} from addr: {:?}, reason: {:?}", port, addr, e)
            }
        });
    }
}

pub async fn start(context: Arc<AppContext>) -> Result<()> {
    let mut ports = JoinSet::new();
    for app in context.config.apps() {
        for port in app.ports() {
            let context_ref = context.clone();
            let app_ref = app.clone();
            let port_ref = *port;
            let port_listener = async move {
                start_port_listener(context_ref, app_ref, port_ref).await
            };

            ports.spawn(port_listener);
        }
    }

    if let Some(join_res) = ports.join_next().await {
        any_ok!(join_res)
    } else {
        tracing::info!("no ports started, exiting...");
        Ok(())
    }
}
