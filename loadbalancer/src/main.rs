use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tracing_log::AsTrace;

use crate::config::Config;
use crate::context::{AppContext, Cli};

mod config;
mod conn;
mod consts;
mod context;
mod core;
mod util;
mod transfer;
mod copy;

fn setup_tracing(cli: &Cli) -> Result<()> {
    let level: tracing::Level = cli
        .verbose
        .log_level()
        .map(|level| level.as_trace())
        .unwrap_or_else(|| tracing::Level::INFO);

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .with_context(|| "failed to setup tracing global subscriber")?;

    std::panic::set_hook(Box::new(|panic| {
        if let Some(location) = panic.location() {
            tracing::error!(
                message = %panic,
                panic.file = location.file(),
                panic.line = location.line(),
                panic.column = location.column(),
            );
        } else {
            tracing::error!(message = %panic);
        }
    }));

    Ok(())
}

async fn do_main(cli: Cli) -> Result<()> {
    util::setup_limits();
    setup_tracing(&cli)?;

    tracing::info!("building application context...");
    let context = AppContext::new(cli).await?;

    tracing::info!(
        "starting application with cli: {:?} and config: {:?}",
        context.cli,
        context.config
    );
    core::start(Arc::new(context)).await
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(e) = do_main(cli).await {
        tracing::error!("application failure: {:?}", e);
    }
}
