use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use rand::prelude::SliceRandom;
use tokio::net::TcpStream;

use crate::{consts, util};
use crate::config::AppConfig;

enum ConnectionResolver {
    RoundRobin(RoundRobinConnectionResolver),
}

struct RoundRobinConnectionResolver {
    idx: AtomicUsize,
    hosts: Vec<String>,
}

impl RoundRobinConnectionResolver {
    pub fn new(hosts: Vec<String>) -> Self {
        Self {
            idx: AtomicUsize::new(0),
            hosts,
        }
    }

    pub async fn connect_next(&self) -> Result<TcpStream> {
        let idx = self.idx.fetch_add(1, Ordering::SeqCst) % self.hosts.len();
        let host = &self.hosts[idx];

        TcpStream::connect(&host).await.map_err(anyhow::Error::from)
    }
}

impl ConnectionResolver {
    pub async fn connect_next(&self) -> Result<TcpStream> {
        match self {
            ConnectionResolver::RoundRobin(x) => x.connect_next().await,
        }
    }
}

pub async fn connect(app_config: &AppConfig) -> Result<TcpStream> {
    let mut targets = app_config.targets().clone();
    {
        let mut rng = rand::thread_rng();
        targets.shuffle(&mut rng);
    }

    let connection_resolver = ConnectionResolver::RoundRobin(
        RoundRobinConnectionResolver::new(targets));

    util::retry_immediate(
        consts::TARGET_CONNECT_RETRIES,
        |idx| {
            tracing::info!("attempting to connect, attempt#: {}", idx);
            connection_resolver.connect_next()
        },
    ).await
}