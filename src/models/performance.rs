// src/models/performance.rs

pub trait PerformancePredictor {
    /// Calculates predicted tokens per second
    /// Formula: Bandwidth (GB/s) / Model Size (GB)
    fn predict_tps(&self, weight_gb: f64, bandwidth_gbps: f64) -> f64;
}

pub struct PerformanceEngine;

impl PerformancePredictor for PerformanceEngine {
    fn predict_tps(&self, weight_gb: f64, bandwidth_gbps: f64) -> f64 {
        if weight_gb <= 0.0 {
            return 0.0;
        }

        // At batch size 1, we are strictly bandwidth bound.
        // We load the entire model weight set for every single token generated.
        let raw_tps = bandwidth_gbps / weight_gb;

        // Apply a "Real-World Efficiency" penalty (usually ~80% of theoretical)
        raw_tps * 0.85
    }
}
