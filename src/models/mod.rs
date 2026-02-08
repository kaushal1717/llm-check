use serde::{Deserialize, Serialize};

// 1. Define the shared data structure
#[derive(Deserialize, Serialize)]
pub struct ModelMetadata {
    #[serde(alias = "num_hidden_layers", alias = "n_layer")]
    pub layers: u32,
    #[serde(alias = "hidden_size", alias = "n_embd")]
    pub hidden_size: u32,
    #[serde(alias = "num_key_value_heads", default)]
    pub kv_heads: f64,
    #[serde(alias = "num_attention_heads", alias = "n_head", default)]
    pub heads: f64,
    #[serde(alias = "num_local_experts", alias = "n_routed_experts", default)]
    pub num_experts: Option<u32>,
    #[serde(alias = "num_experts_per_tok", default)]
    pub experts_per_token: Option<u32>,
    #[serde(alias = "intermediate_size", default)]
    pub intermediate_size: Option<u32>,
}

impl ModelMetadata {
    /// Returns true if this is a Mixture of Experts model
    pub fn is_moe(&self) -> bool {
        self.num_experts.is_some_and(|n| n > 1)
    }

    /// Returns the ratio of active params to total params per token.
    /// For MoE: experts_per_token / num_experts (applied to expert portion only).
    /// For dense: 1.0
    pub fn active_params_ratio(&self) -> f64 {
        match (self.num_experts, self.experts_per_token) {
            (Some(total), Some(active)) if total > 1 => active as f64 / total as f64,
            _ => 1.0,
        }
    }
}

#[derive(Serialize)]
pub struct Suggestion {
    pub model_id: String,
    pub name: String,
    pub params_bn: f64,
}

pub const RECOMMENDATION_POOL: &[Suggestion] = &[];

// 2. Declare the sub-files as modules
pub mod eco;
mod estimator;
mod fetcher;
mod performance;

// 3. "Export" them so they are accessible from models::...
pub use eco::{EcoReport, calculate_eco, SUPPORTED_REGIONS};
pub use estimator::VramEstimator;
pub use fetcher::{fetch_metadata, discover_models};
pub use performance::{PerformanceEngine, PerformancePredictor};
