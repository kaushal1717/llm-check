// src/config/mod.rs
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub hf_token: Option<String>,
    pub benchmark_bandwidth_gbps: Option<f64>,
    /// Carbon intensity region (e.g. "US", "EU", "CN", "NO")
    pub carbon_region: Option<String>,
    /// Override carbon intensity in g CO2/kWh
    pub carbon_intensity_override: Option<f64>,
    /// Electricity cost in USD per kWh
    pub energy_cost_per_kwh: Option<f64>,
}

impl AppConfig {
    /// Find the system-standard config path
    fn get_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("llm-check");
        fs::create_dir_all(&path).ok();
        path.push("config.toml");
        path
    }

    /// Load config from disk
    pub fn load() -> Self {
        let path = Self::get_path();
        if let Ok(content) = fs::read_to_string(path) {
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Save current config to disk
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::get_path();
        let content = toml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Returns the HF token with priority: config file > HF_TOKEN env var
    pub fn get_token(&self) -> Option<String> {
        self.hf_token
            .clone()
            .or_else(|| std::env::var("HF_TOKEN").ok())
    }

    /// Returns the config file path for display purposes
    pub fn config_path() -> PathBuf {
        Self::get_path()
    }
}
