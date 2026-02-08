// src/hardware/bench.rs
use serde::Serialize;
use std::time::Instant;

#[derive(Serialize)]
pub struct BenchResult {
    pub bandwidth_gbps: f64,
    pub buffer_size_mb: usize,
    pub iterations: usize,
    pub total_duration_secs: f64,
}

/// Measure memory bandwidth by copying a large buffer repeatedly.
/// Uses 256 MB buffer, runs `iterations` times, takes the median.
pub fn measure_bandwidth(iterations: usize) -> BenchResult {
    let buffer_size: usize = 256 * 1024 * 1024; // 256 MB

    // Allocate two buffers
    let src = vec![1u8; buffer_size];
    let mut dst = vec![0u8; buffer_size];

    // Warmup (1 iteration)
    dst.copy_from_slice(&src);
    std::hint::black_box(&dst);

    // Timed runs
    let mut timings = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        dst.copy_from_slice(&src);
        let elapsed = start.elapsed();
        std::hint::black_box(&dst);
        timings.push(elapsed.as_secs_f64());
    }

    // Use median timing
    timings.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_secs = timings[iterations / 2];

    // Bandwidth = bytes_copied / time (copy reads src + writes dst)
    let bytes_per_sec = buffer_size as f64 / median_secs;
    let gbps = bytes_per_sec / 1e9;

    BenchResult {
        bandwidth_gbps: gbps,
        buffer_size_mb: buffer_size / (1024 * 1024),
        iterations,
        total_duration_secs: timings.iter().sum(),
    }
}
