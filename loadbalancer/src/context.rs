use anyhow::Result;
use clap::Parser;
use tokio::time::Duration;

use crate::{config, Config, consts};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// configuration path, json expected
    #[arg(short, long)]
    pub config_path: String,

    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    #[arg(long)]
    pub target_connect_timeout_ms: Option<u64>,

    #[arg(long)]
    pub inactivity_timeout_ms: Option<u64>,
}

pub struct AppContext {
    pub cli: Cli,
    pub config: Config,
}

impl Cli {
    pub fn target_connect_timeout(&self) -> Duration {
        Duration::from_millis(self.target_connect_timeout_ms.unwrap_or(consts::TARGET_CONNECT_TIMEOUT_MS))
    }

    pub fn inactivity_timeout(&self) -> Duration {
        Duration::from_millis(self.inactivity_timeout_ms.unwrap_or(consts::INACTIVITY_TIMEOUT_MS))
    }
}

impl AppContext {
    pub async fn new(cli: Cli) -> Result<Self> {
        let config = config::read_config(&cli.config_path).await?;

        Ok(AppContext { config, cli })
    }
}
