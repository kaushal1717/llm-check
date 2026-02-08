// src/hardware/mod.rs
pub mod bench;

use nvml_wrapper::Nvml;
use serde::Serialize;
use sysinfo::System;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AcceleratorType {
    None,
    NvidiaGpu,
    AppleSilicon,
}

/// Per-GPU detailed info for topology reporting
#[derive(Debug, Clone, Serialize)]
pub struct GpuInfo {
    pub index: u32,
    pub name: String,
    pub vram_gb: f64,
    pub pcie_gen: Option<u32>,
    pub pcie_width: Option<u32>,
    pub max_pcie_gen: Option<u32>,
    pub max_pcie_width: Option<u32>,
    pub pcie_bottleneck: bool,
    pub nvlink_active: bool,
    pub nvlink_version: Option<u32>,
    pub tdp_watts: Option<f64>,
    pub bandwidth_gbps: f64,
}

/// Diagnostic results for the system
#[derive(Debug, Clone, Serialize)]
pub struct SystemSpecs {
    pub ram_gb: f64,
    pub cpu_brand: String,
    pub gpu_name: Option<String>,
    pub vram_gb: Option<f64>,
    pub bandwidth_gbps: f64,
    pub tdp_watts: Option<f64>,
    pub accelerator_type: AcceleratorType,
    pub npu_available: bool,
    pub unified_memory: bool,
    pub gpu_count: u32,
    pub gpus: Vec<GpuInfo>,
}

/// Apple Silicon detection result
#[cfg(target_os = "macos")]
struct AppleSiliconInfo {
    chip_name: String,
    gpu_cores: Option<u32>,
    bandwidth_gbps: f64,
    tdp_watts: f64,
}

#[cfg(target_os = "macos")]
fn detect_apple_silicon() -> Option<AppleSiliconInfo> {
    // 1. Get CPU brand string via sysctl
    let cpu_output = std::process::Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok()?;

    let cpu_brand = String::from_utf8_lossy(&cpu_output.stdout).trim().to_string();

    // Only proceed if this is Apple Silicon
    if !cpu_brand.starts_with("Apple") {
        return None;
    }

    // 2. Get GPU core count from system_profiler
    let gpu_cores = get_gpu_core_count();

    // 3. Determine bandwidth from chip name
    let bandwidth_gbps = match cpu_brand.as_str() {
        s if s.contains("M1 Ultra") => 800.0,
        s if s.contains("M1 Max") => 400.0,
        s if s.contains("M1 Pro") => 200.0,
        s if s.contains("M1") => 68.25,
        s if s.contains("M2 Ultra") => 800.0,
        s if s.contains("M2 Max") => 400.0,
        s if s.contains("M2 Pro") => 200.0,
        s if s.contains("M2") => 100.0,
        s if s.contains("M3 Ultra") => 800.0,
        s if s.contains("M3 Max") => 400.0,
        s if s.contains("M3 Pro") => 150.0,
        s if s.contains("M3") => 100.0,
        s if s.contains("M4 Max") => 546.0,
        s if s.contains("M4 Pro") => 273.0,
        s if s.contains("M4") => 120.0,
        _ => 100.0, // Conservative default for future Apple chips
    };

    // 4. TDP (package power under ML workload)
    let tdp_watts = match cpu_brand.as_str() {
        s if s.contains("M1 Ultra") => 60.0,
        s if s.contains("M1 Max") => 40.0,
        s if s.contains("M1 Pro") => 33.0,
        s if s.contains("M1") => 15.0,
        s if s.contains("M2 Ultra") => 60.0,
        s if s.contains("M2 Max") => 40.0,
        s if s.contains("M2 Pro") => 33.0,
        s if s.contains("M2") => 15.0,
        s if s.contains("M3 Ultra") => 60.0,
        s if s.contains("M3 Max") => 40.0,
        s if s.contains("M3 Pro") => 33.0,
        s if s.contains("M3") => 15.0,
        s if s.contains("M4 Max") => 40.0,
        s if s.contains("M4 Pro") => 33.0,
        s if s.contains("M4") => 15.0,
        _ => 20.0,
    };

    Some(AppleSiliconInfo {
        chip_name: cpu_brand,
        gpu_cores,
        bandwidth_gbps,
        tdp_watts,
    })
}

#[cfg(target_os = "macos")]
fn get_gpu_core_count() -> Option<u32> {
    let output = std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .output()
        .ok()?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let displays = json.get("SPDisplaysDataType")?.as_array()?;
    let gpu = displays.first()?;

    // Apple reports GPU cores in "sppci_cores" field
    gpu.get("sppci_cores")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u32>().ok())
}

pub fn scan_components() -> SystemSpecs {
    scan_components_with_config(None)
}

pub fn scan_components_with_config(stored_bandwidth: Option<f64>) -> SystemSpecs {
    let mut sys = System::new_all();
    sys.refresh_all();

    // RAM
    let total_bytes = sys.total_memory();
    let ram_gb = total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    // CPU
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    // Try Apple Silicon first (macOS only)
    #[cfg(target_os = "macos")]
    if let Some(apple) = detect_apple_silicon() {
        let gpu_label = match apple.gpu_cores {
            Some(cores) => format!("{} ({}-core GPU)", apple.chip_name, cores),
            None => apple.chip_name.clone(),
        };

        return SystemSpecs {
            ram_gb,
            cpu_brand: apple.chip_name,
            gpu_name: Some(gpu_label),
            vram_gb: Some(ram_gb),
            bandwidth_gbps: apple.bandwidth_gbps,
            tdp_watts: Some(apple.tdp_watts),
            accelerator_type: AcceleratorType::AppleSilicon,
            npu_available: true,
            unified_memory: true,
            gpu_count: 1,
            gpus: vec![],
        };
    }

    // Fall back to NVIDIA GPU detection
    let mut gpu_name = None;
    let mut vram_gb = None;
    let mut accelerator_type = AcceleratorType::None;
    let mut lut_bandwidth: Option<f64> = None;
    let mut tdp_watts: Option<f64> = None;

    if let Ok(nvml) = Nvml::init() {
        if let Ok(device) = nvml.device_by_index(0) {
            let name = device.name().unwrap_or_default();
            lut_bandwidth = nvidia_bandwidth_lut(&name);
            tdp_watts = nvidia_tdp_lut(&name);

            gpu_name = Some(name);
            accelerator_type = AcceleratorType::NvidiaGpu;
            if let Ok(mem) = device.memory_info() {
                vram_gb = Some(mem.total as f64 / (1024.0 * 1024.0 * 1024.0));
            }
        }
    }

    // Bandwidth priority: GPU-specific LUT > stored benchmark > conservative default
    let bandwidth = lut_bandwidth
        .or(stored_bandwidth)
        .unwrap_or(50.0);

    // Count GPUs
    let gpu_count = if let Ok(nvml) = Nvml::init() {
        nvml.device_count().unwrap_or(if gpu_name.is_some() { 1 } else { 0 })
    } else if gpu_name.is_some() {
        1
    } else {
        0
    };

    SystemSpecs {
        ram_gb,
        cpu_brand,
        gpu_name,
        vram_gb,
        bandwidth_gbps: bandwidth,
        tdp_watts,
        accelerator_type,
        npu_available: false,
        unified_memory: false,
        gpu_count,
        gpus: vec![],
    }
}

fn nvidia_bandwidth_lut(name: &str) -> Option<f64> {
    match name {
        n if n.contains("4090") => Some(1008.0),
        n if n.contains("4080") => Some(716.0),
        n if n.contains("4070") => Some(504.0),
        n if n.contains("3090") => Some(936.0),
        n if n.contains("3080") => Some(760.0),
        n if n.contains("3060") => Some(360.0),
        n if n.contains("A100") => Some(1935.0),
        n if n.contains("H100") => Some(3350.0),
        _ => None,
    }
}

fn nvidia_tdp_lut(name: &str) -> Option<f64> {
    match name {
        n if n.contains("4090") => Some(450.0),
        n if n.contains("4080") => Some(320.0),
        n if n.contains("4070") => Some(200.0),
        n if n.contains("3090") => Some(350.0),
        n if n.contains("3080") => Some(320.0),
        n if n.contains("3060") => Some(170.0),
        n if n.contains("A100") => Some(300.0),
        n if n.contains("H100") => Some(700.0),
        _ => None,
    }
}

/// Topology report for multi-GPU systems
#[derive(Debug, Clone, Serialize)]
pub struct TopologyReport {
    pub gpu_count: u32,
    pub total_vram_gb: f64,
    pub gpus: Vec<GpuInfo>,
}

/// Scan all NVIDIA GPUs with PCIe and NVLink details.
/// Returns None on Apple Silicon or when no NVIDIA GPUs are found.
pub fn scan_topology(stored_bandwidth: Option<f64>) -> Option<TopologyReport> {
    let nvml = Nvml::init().ok()?;
    let count = nvml.device_count().ok()?;
    if count == 0 {
        return None;
    }

    let mut gpus = Vec::new();
    let mut total_vram = 0.0;

    for idx in 0..count {
        let device = match nvml.device_by_index(idx) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let name = device.name().unwrap_or_else(|_| format!("GPU {}", idx));
        let vram_gb = device
            .memory_info()
            .map(|m| m.total as f64 / (1024.0 * 1024.0 * 1024.0))
            .unwrap_or(0.0);
        total_vram += vram_gb;

        // PCIe info
        let pcie_gen = device.current_pcie_link_gen().ok();
        let pcie_width = device.current_pcie_link_width().ok();
        let max_pcie_gen = device.max_pcie_link_gen().ok();
        let max_pcie_width = device.max_pcie_link_width().ok();

        let pcie_bottleneck = match (pcie_gen, max_pcie_gen, pcie_width, max_pcie_width) {
            (Some(cg), Some(mg), Some(cw), Some(mw)) => cg < mg || cw < mw,
            _ => false,
        };

        // NVLink probe (check links 0-5)
        let mut nvlink_active = false;
        let mut nvlink_version = None;
        for link_id in 0..6 {
            let link = device.link_wrapper_for(link_id);
            if link.is_active().unwrap_or(false) {
                nvlink_active = true;
                if nvlink_version.is_none() {
                    nvlink_version = link.version().ok();
                }
                break;
            }
        }

        let bandwidth_gbps = nvidia_bandwidth_lut(&name)
            .or(stored_bandwidth)
            .unwrap_or(50.0);
        let tdp_watts = nvidia_tdp_lut(&name);

        gpus.push(GpuInfo {
            index: idx,
            name,
            vram_gb,
            pcie_gen,
            pcie_width,
            max_pcie_gen,
            max_pcie_width,
            pcie_bottleneck,
            nvlink_active,
            nvlink_version,
            tdp_watts,
            bandwidth_gbps,
        });
    }

    Some(TopologyReport {
        gpu_count: count,
        total_vram_gb: total_vram,
        gpus,
    })
}
