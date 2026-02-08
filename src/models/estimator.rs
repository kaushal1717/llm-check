// src/models/estimator.rs
use crate::models::ModelMetadata;

pub trait VramEstimator {
    fn estimate_gb(&self, quant: &str, context_len: u32) -> f64;
}

impl VramEstimator for ModelMetadata {
    fn estimate_gb(&self, quant: &str, context_len: u32) -> f64 {
        // 1. Determine bits per weight
        let b_w = match quant.to_lowercase().as_str() {
            "fp16" | "bf16" => 2.0,
            "int8" => 1.0,
            "q4_k_m" | "int4" => 0.57, // empirically validated for GGUF
            _ => 0.57,
        };

        // 2. Estimate Parameters (P in Billions)
        let params_bn = if self.is_moe() {
            // MoE: shared attention params + ALL expert FFN params
            let h = self.hidden_size as f64;
            let l = self.layers as f64;
            let num_exp = self.num_experts.unwrap_or(1) as f64;
            let inter = self.intermediate_size.unwrap_or(self.hidden_size * 4) as f64;

            // Shared: attention (Q,K,V,O) = 4 * h^2 per layer
            let shared = l * h * h * 4.0;
            // Expert FFN: each expert has gate_proj + up_proj + down_proj = 3 * h * inter
            let experts_total = l * num_exp * 3.0 * h * inter;

            (shared + experts_total) / 1e9
        } else {
            // Dense: standard approximation
            (self.layers as f64 * (self.hidden_size as f64).powi(2) * 12.0) / 1e9
        };

        // 3. Calculate Weight Memory
        let weight_gb = params_bn * b_w * 1.2; // Including 20% buffer

        // 4. Calculate KV Cache
        // Formula: (2 * layers * context * (hidden_size / GQA_factor) * 2 bytes) / 10^9
        let gqa_factor = if self.kv_heads > 0.0 && self.heads > 0.0 {
            self.heads / self.kv_heads
        } else {
            1.0
        };
        let kv_cache_gb = (2.0
            * self.layers as f64
            * context_len as f64
            * (self.hidden_size as f64 / gqa_factor)
            * 2.0)
            / 1e9;

        // 5. Framework/CUDA Overhead
        let overhead_gb = 0.55 + (0.08 * params_bn);

        weight_gb + kv_cache_gb + overhead_gb
    }
}
