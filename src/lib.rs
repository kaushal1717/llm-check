// src/lib.rs

use crate::hardware::AcceleratorType;
use crate::models::RECOMMENDATION_POOL;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, Table};
use owo_colors::OwoColorize;
use serde::Serialize;

pub mod config;
pub mod hardware;
pub mod models;
pub mod ui;
pub mod updater;

/// A high-level wrapper to run the full scan and print results
pub fn display_hardware_report() {
    let sp = ui::spinner("Probing local hardware...");
    let specs = hardware::scan_components();
    sp.finish_and_clear();

    println!("{}", "--- Hardware Report ---".bold().cyan());
    println!("  RAM:  {:.2} GB", specs.ram_gb);
    println!("  CPU:  {}", specs.cpu_brand.dimmed());

    match specs.accelerator_type {
        AcceleratorType::AppleSilicon => {
            let gpu_label = specs.gpu_name.as_deref().unwrap_or("Apple GPU");
            println!(
                "  GPU:  {} (Unified Memory: {:.2} GB)",
                gpu_label.green(),
                specs.ram_gb
            );
            println!("  BW:   {:.1} GB/s", specs.bandwidth_gbps);
            if specs.npu_available {
                println!("  NPU:  {}", "Neural Engine available".green());
            }
        }
        AcceleratorType::NvidiaGpu => {
            if let (Some(name), Some(vram)) = (&specs.gpu_name, specs.vram_gb) {
                println!("  GPU:  {} ({:.2} GB VRAM)", name.green(), vram);
                println!("  BW:   {:.1} GB/s", specs.bandwidth_gbps);
            }
        }
        AcceleratorType::None => {
            println!("  GPU:  {}", "No supported accelerator detected.".yellow());
        }
    }
    if specs.gpu_count > 1 {
        println!(
            "  GPUs: {} devices detected (use `llm-check topology` for details)",
            specs.gpu_count.to_string().green()
        );
    }
    println!("{}", "-----------------------".dimmed());
}

pub fn get_smart_recommendation(user_vram_gb: f64) -> Vec<String> {
    let mut suggestions = Vec::new();

    for item in RECOMMENDATION_POOL {
        let weight_gb = item.params_bn * 0.57 * 1.2;

        if weight_gb <= user_vram_gb {
            suggestions.push(format!(
                "  {} {} ({})",
                "[match]".green(),
                item.name,
                item.model_id
            ));
        }
    }

    suggestions
}

/// moe_info: Optional (num_experts, experts_per_token, active_weight_gb)
pub fn print_verdict_table(
    model_name: &str,
    weight_gb: f64,
    vram_needed: f64,
    available_vram: f64,
    tps: f64,
    moe_info: Option<(u32, u32, f64)>,
) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Metric").fg(Color::Cyan),
            Cell::new("Value").fg(Color::Cyan),
        ]);

    let (vram_color, verdict_text) = if available_vram >= vram_needed {
        (Color::Green, "PERFECT FIT")
    } else if (available_vram + 4.0) >= vram_needed {
        (Color::Yellow, "OFFLOADING (SLOW)")
    } else {
        (Color::Red, "OUT OF MEMORY")
    };

    if let Some((num_exp, active_exp, active_wt)) = moe_info {
        table.add_row(vec![
            Cell::new("Architecture"),
            Cell::new(format!("MoE ({}/{} experts active)", active_exp, num_exp))
                .fg(Color::Magenta),
        ]);
        table.add_row(vec![
            Cell::new("Total Weight (GB)"),
            Cell::new(format!("{:.2}", weight_gb)).fg(Color::White),
        ]);
        table.add_row(vec![
            Cell::new("Active Weight (GB)"),
            Cell::new(format!("{:.2}", active_wt)).fg(Color::Green),
        ]);
    } else {
        table.add_row(vec![
            Cell::new("Estimated Weight (GB)"),
            Cell::new(format!("{:.2}", weight_gb)).fg(Color::White),
        ]);
    }

    table.add_row(vec![
        Cell::new("VRAM Needed (GB)"),
        Cell::new(format!("{:.2}", vram_needed)).fg(Color::White),
    ]);

    table.add_row(vec![
        Cell::new("Available VRAM (GB)"),
        Cell::new(format!("{:.2}", available_vram)).fg(Color::White),
    ]);

    table.add_row(vec![
        Cell::new("Predicted Speed (tokens/sec)"),
        Cell::new(format!("{:.1}", tps)).fg(Color::White),
    ]);

    table.add_row(vec![
        Cell::new("Verdict"),
        Cell::new(verdict_text).fg(vram_color),
    ]);

    println!(
        "\n{} {}\n{}",
        "Final Verdict for:".bold(),
        model_name.cyan(),
        table
    );
}

#[derive(Serialize)]
pub struct VerdictReport {
    pub model: String,
    pub architecture: String,
    pub weight_gb: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_weight_gb: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_experts: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experts_per_token: Option<u32>,
    pub vram_needed_gb: f64,
    pub vram_available_gb: f64,
    pub predicted_tps: f64,
    pub verdict: String,
}

pub fn build_verdict(
    model_name: &str,
    weight_gb: f64,
    vram_needed: f64,
    available_vram: f64,
    tps: f64,
    moe_info: Option<(u32, u32, f64)>,
) -> VerdictReport {
    let verdict = if available_vram >= vram_needed {
        "PERFECT_FIT"
    } else if (available_vram + 4.0) >= vram_needed {
        "OFFLOADING"
    } else {
        "OUT_OF_MEMORY"
    };

    let (architecture, active_weight_gb, num_experts, experts_per_token) = match moe_info {
        Some((ne, ept, aw)) => ("moe".to_string(), Some(aw), Some(ne), Some(ept)),
        None => ("dense".to_string(), None, None, None),
    };

    VerdictReport {
        model: model_name.to_string(),
        architecture,
        weight_gb,
        active_weight_gb,
        num_experts,
        experts_per_token,
        vram_needed_gb: vram_needed,
        vram_available_gb: available_vram,
        predicted_tps: tps,
        verdict: verdict.to_string(),
    }
}

pub fn print_eco_table(eco: &models::EcoReport) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Eco Metric").fg(Color::Cyan),
            Cell::new("Value").fg(Color::Cyan),
        ]);

    table.add_row(vec![
        Cell::new("GPU Power (TDP)"),
        Cell::new(format!("{:.0} W", eco.tdp_watts)).fg(Color::White),
    ]);

    table.add_row(vec![
        Cell::new("Energy per Token"),
        Cell::new(format!("{:.2} J", eco.joules_per_token)).fg(Color::White),
    ]);

    table.add_row(vec![
        Cell::new("Energy per 1K Tokens"),
        Cell::new(format!("{:.6} kWh", eco.kwh_per_1k_tokens)).fg(Color::White),
    ]);

    let co2_color = if eco.co2_grams_per_1k_tokens < 0.05 {
        Color::Green
    } else if eco.co2_grams_per_1k_tokens < 0.2 {
        Color::Yellow
    } else {
        Color::Red
    };

    table.add_row(vec![
        Cell::new("CO2 per 1K Tokens"),
        Cell::new(format!("{:.4} g", eco.co2_grams_per_1k_tokens)).fg(co2_color),
    ]);

    table.add_row(vec![
        Cell::new("Cost per 1K Tokens"),
        Cell::new(format!("${:.6}", eco.cost_usd_per_1k_tokens)).fg(Color::White),
    ]);

    table.add_row(vec![
        Cell::new("Region"),
        Cell::new(format!(
            "{} ({:.0} g CO2/kWh)",
            eco.region, eco.carbon_intensity_g_per_kwh
        ))
        .fg(Color::White),
    ]);

    println!("\n{}\n{}", "Eco Impact Report".bold().green(), table);
}

pub fn print_topology_table(report: &hardware::TopologyReport) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("GPU").fg(Color::Cyan),
            Cell::new("Name").fg(Color::Cyan),
            Cell::new("VRAM").fg(Color::Cyan),
            Cell::new("PCIe").fg(Color::Cyan),
            Cell::new("NVLink").fg(Color::Cyan),
            Cell::new("BW (GB/s)").fg(Color::Cyan),
            Cell::new("TDP").fg(Color::Cyan),
        ]);

    for gpu in &report.gpus {
        let pcie_str = match (gpu.pcie_gen, gpu.pcie_width) {
            (Some(g), Some(width)) => format!("Gen{} x{}", g, width),
            _ => "N/A".to_string(),
        };
        let pcie_color = if gpu.pcie_bottleneck {
            Color::Yellow
        } else {
            Color::White
        };

        let nvlink_str = if gpu.nvlink_active {
            match gpu.nvlink_version {
                Some(v) => format!("v{}", v),
                None => "Active".to_string(),
            }
        } else {
            "---".to_string()
        };
        let nvlink_color = if gpu.nvlink_active {
            Color::Green
        } else {
            Color::DarkGrey
        };

        let tdp_str = match gpu.tdp_watts {
            Some(w) => format!("{:.0}W", w),
            None => "N/A".to_string(),
        };

        table.add_row(vec![
            Cell::new(gpu.index),
            Cell::new(&gpu.name).fg(Color::White),
            Cell::new(format!("{:.2} GB", gpu.vram_gb)).fg(Color::White),
            Cell::new(&pcie_str).fg(pcie_color),
            Cell::new(&nvlink_str).fg(nvlink_color),
            Cell::new(format!("{:.1}", gpu.bandwidth_gbps)).fg(Color::White),
            Cell::new(&tdp_str).fg(Color::White),
        ]);
    }

    println!("\n{}\n{}", "GPU Topology".bold().cyan(), table);
    println!(
        "  {} {} GPUs | Total VRAM: {:.2} GB",
        "Summary:".bold(),
        report.gpu_count,
        report.total_vram_gb
    );

    // Warn about PCIe bottlenecks
    for gpu in &report.gpus {
        if gpu.pcie_bottleneck {
            println!(
                "  {} GPU {} running at PCIe Gen{} x{} (max: Gen{} x{})",
                "[bottleneck]".yellow(),
                gpu.index,
                gpu.pcie_gen.unwrap_or(0),
                gpu.pcie_width.unwrap_or(0),
                gpu.max_pcie_gen.unwrap_or(0),
                gpu.max_pcie_width.unwrap_or(0),
            );
        }
    }
}
