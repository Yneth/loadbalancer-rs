use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Ports")]
    ports: Vec<u16>,
    #[serde(rename = "Targets")]
    targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "Apps")]
    apps: Vec<AppConfig>,
}

impl AppConfig {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ports(&self) -> &Vec<u16> {
        &self.ports
    }

    pub fn targets(&self) -> &Vec<String> {
        &self.targets
    }
}

impl Config {
    pub fn apps(&self) -> &Vec<AppConfig> {
        &self.apps
    }
}

pub async fn read_config(uri: &str) -> Result<Config> {
    if tokio::fs::metadata(&uri).await.is_ok() {
        let buf = tokio::fs::read(uri)
            .await
            .with_context(|| format!("failed to read config file path: {}", uri))?;

        serde_json::from_slice(&buf)
            .with_context(|| "failed to parse config file".to_string())
    } else {
        serde_json::from_slice(uri.as_bytes())
            .with_context(|| "failed to parse config".to_string())
    }
}
