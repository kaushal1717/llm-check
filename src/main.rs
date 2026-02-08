// src/main.rs
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use llm_check::config::AppConfig;
use llm_check::models::{
    PerformanceEngine, PerformancePredictor, SUPPORTED_REGIONS, VramEstimator, calculate_eco,
    discover_models, fetch_metadata,
};
use llm_check::{display_hardware_report, hardware, ui};
use owo_colors::OwoColorize;

#[derive(Clone, Debug, Default, ValueEnum)]
enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(Parser)]
#[command(name = "llm-check")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format: text or json
    #[arg(long, global = true, default_value = "text", value_enum)]
    format: OutputFormat,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan local hardware (CPU, GPU, RAM)
    Scan,
    /// Check if a specific model fits your hardware
    Check {
        model_id: String,
        #[arg(short, long, default_value = "q4_k_m")]
        quant: String,
        /// Show energy & carbon impact report
        #[arg(long)]
        eco: bool,
    },
    /// Suggest compatible models for your hardware
    Suggest,
    /// Show multi-GPU topology and PCIe/NVLink status
    Topology,
    /// Manage configuration (token, settings)
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Generate shell completion scripts
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, elvish, powershell)
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Update llm-check to the latest version
    Update {
        /// Only check for updates, don't install
        #[arg(long)]
        check_only: bool,
    },
    /// Measure memory bandwidth and save result
    Bench {
        /// Number of iterations
        #[arg(short, long, default_value = "10")]
        iterations: usize,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Save your Hugging Face token globally
    SetToken { token: String },
    /// Set carbon intensity region (e.g. US, EU, NO)
    SetRegion { region: String },
    /// Show current configuration status
    Status,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let mut config = AppConfig::load();
    dotenvy::dotenv().ok();

    let token = config.get_token();
    let token_ref = token.as_deref();
    let is_json = matches!(cli.format, OutputFormat::Json);
    let stored_bw = config.benchmark_bandwidth_gbps;

    match &cli.command {
        Commands::Scan => {
            if is_json {
                let specs = hardware::scan_components_with_config(stored_bw);
                println!("{}", serde_json::to_string_pretty(&specs).unwrap());
            } else {
                display_hardware_report();
            }
        }
        Commands::Check {
            model_id,
            quant,
            eco,
        } => {
            let sp = if !is_json {
                Some(ui::spinner(&format!(
                    "Fetching metadata for {}...",
                    model_id
                )))
            } else {
                None
            };

            match fetch_metadata(model_id, token_ref).await {
                Ok(meta) => {
                    if let Some(s) = sp {
                        s.finish_with_message(format!(
                            "{} Model metadata retrieved",
                            "[ok]".green()
                        ));
                    }

                    let specs = if !is_json {
                        let sp2 = ui::spinner("Scanning hardware...");
                        let s = hardware::scan_components_with_config(stored_bw);
                        sp2.finish_and_clear();
                        s
                    } else {
                        hardware::scan_components_with_config(stored_bw)
                    };

                    let vram_avail = specs.vram_gb.unwrap_or(0.0);
                    let needed = meta.estimate_gb(quant, 2048);
                    let weight_only = meta.estimate_gb(quant, 0) - 1.0;

                    // For MoE, TPS is based on active params only (sparse activation)
                    let active_weight = if meta.is_moe() {
                        weight_only * meta.active_params_ratio()
                    } else {
                        weight_only
                    };

                    let engine = PerformanceEngine;
                    let tps = engine.predict_tps(active_weight, specs.bandwidth_gbps);

                    let moe_info = if meta.is_moe() {
                        Some((
                            meta.num_experts.unwrap_or(0),
                            meta.experts_per_token.unwrap_or(0),
                            active_weight,
                        ))
                    } else {
                        None
                    };

                    if is_json {
                        let report = llm_check::build_verdict(
                            model_id,
                            weight_only,
                            needed,
                            vram_avail,
                            tps,
                            moe_info,
                        );
                        println!("{}", serde_json::to_string_pretty(&report).unwrap());
                    } else {
                        llm_check::print_verdict_table(
                            model_id,
                            weight_only,
                            needed,
                            vram_avail,
                            tps,
                            moe_info,
                        );
                    }

                    if *eco {
                        let tdp = specs.tdp_watts.unwrap_or(150.0);
                        let region = config.carbon_region.as_deref().unwrap_or("US");
                        let eco_report = calculate_eco(
                            tdp,
                            tps,
                            region,
                            config.carbon_intensity_override,
                            config.energy_cost_per_kwh,
                        );
                        if is_json {
                            println!("{}", serde_json::to_string_pretty(&eco_report).unwrap());
                        } else {
                            llm_check::print_eco_table(&eco_report);
                        }
                    }
                }
                Err(e) => {
                    if let Some(s) = sp {
                        s.finish_with_message(format!(
                            "{} Failed to fetch model: {}",
                            "[error]".red(),
                            e
                        ));
                    } else {
                        eprintln!("{{\"error\": \"{}\"}}", e);
                    }
                }
            }
        }
        Commands::Suggest => {
            let specs = if !is_json {
                let sp = ui::spinner("Scanning hardware...");
                let s = hardware::scan_components_with_config(stored_bw);
                sp.finish_and_clear();
                s
            } else {
                hardware::scan_components_with_config(stored_bw)
            };

            let vram = if specs.unified_memory {
                specs.ram_gb * 0.75
            } else {
                specs.vram_gb.unwrap_or(specs.ram_gb * 0.5)
            };

            let sp = if !is_json {
                Some(ui::spinner("Discovering models on Hugging Face..."))
            } else {
                None
            };
            match discover_models(token_ref).await {
                Ok(pool) => {
                    if let Some(s) = sp {
                        s.finish_with_message(format!(
                            "{} Found {} models. Checking compatibility with {:.2} GB...",
                            "[ok]".green(),
                            pool.len(),
                            vram
                        ));
                    }

                    let matches: Vec<_> = pool
                        .into_iter()
                        .filter(|item| item.params_bn * 0.57 * 1.2 <= vram)
                        .collect();

                    if is_json {
                        println!("{}", serde_json::to_string_pretty(&matches).unwrap());
                    } else {
                        for item in &matches {
                            println!(
                                "  {} {} ({})",
                                "[match]".green(),
                                item.name,
                                item.model_id.dimmed()
                            );
                        }
                    }
                }
                Err(e) => {
                    if let Some(s) = sp {
                        s.finish_with_message(format!(
                            "{} Discovery failed: {}",
                            "[error]".red(),
                            e
                        ));
                    } else {
                        eprintln!("{{\"error\": \"{}\"}}", e);
                    }
                }
            }
        }
        Commands::Topology => {
            let sp = if !is_json {
                Some(ui::spinner("Scanning GPU topology..."))
            } else {
                None
            };

            match hardware::scan_topology(stored_bw) {
                Some(report) => {
                    if let Some(s) = sp {
                        s.finish_with_message(format!(
                            "{} Found {} GPU(s)",
                            "[ok]".green(),
                            report.gpu_count
                        ));
                    }
                    if is_json {
                        println!("{}", serde_json::to_string_pretty(&report).unwrap());
                    } else {
                        llm_check::print_topology_table(&report);
                    }
                }
                None => {
                    if let Some(s) = sp {
                        s.finish_and_clear();
                        println!(
                            "{} No NVIDIA multi-GPU topology detected (Apple Silicon uses unified memory)",
                            "[info]".cyan()
                        );
                    } else {
                        eprintln!("{{\"error\": \"No NVIDIA GPUs detected\"}}");
                    }
                }
            }
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(*shell, &mut cmd, "llm-check", &mut std::io::stdout());
        }
        Commands::Bench { iterations } => {
            let sp = if !is_json {
                Some(ui::spinner("Running bandwidth benchmark..."))
            } else {
                None
            };

            let result = hardware::bench::measure_bandwidth(*iterations);

            if let Some(s) = sp {
                s.finish_with_message(format!("{} Benchmark complete", "[ok]".green()));
            }

            if is_json {
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
            } else {
                println!("  Measured Bandwidth: {:.2} GB/s", result.bandwidth_gbps);
                println!("  Buffer Size: {} MB", result.buffer_size_mb);
                println!("  Iterations: {}", result.iterations);
                println!("  Total Duration: {:.3}s", result.total_duration_secs);
            }

            // Save to config
            config.benchmark_bandwidth_gbps = Some(result.bandwidth_gbps);
            if let Err(e) = config.save() {
                eprintln!(
                    "{} Failed to save benchmark result: {}",
                    "[warn]".yellow(),
                    e
                );
            } else if !is_json {
                println!("  {} Saved to config for future use", "[ok]".green());
            }
        }
        Commands::Update { check_only } => {
            if *check_only {
                llm_check::updater::check_for_update().await;
            } else {
                llm_check::updater::perform_update().await;
            }
        }
        Commands::Config { action } => match action {
            ConfigAction::SetToken { token } => {
                config.hf_token = Some(token.clone());
                match config.save() {
                    Ok(_) => {
                        println!(
                            "{} Token saved to {}",
                            "[ok]".green(),
                            AppConfig::config_path().display()
                        );
                    }
                    Err(e) => eprintln!("{} Failed to save config: {}", "[error]".red(), e),
                }
            }
            ConfigAction::SetRegion { region } => {
                let upper = region.to_uppercase();
                let valid = SUPPORTED_REGIONS.iter().any(|(code, _)| *code == upper);
                if !valid {
                    eprintln!(
                        "{} Unknown region '{}'. Supported regions:",
                        "[warn]".yellow(),
                        region
                    );
                    for (code, desc) in SUPPORTED_REGIONS {
                        eprintln!("  {} - {}", code, desc);
                    }
                    return;
                }
                config.carbon_region = Some(upper.clone());
                match config.save() {
                    Ok(_) => println!("{} Carbon region set to {}", "[ok]".green(), upper),
                    Err(e) => eprintln!("{} Failed to save config: {}", "[error]".red(), e),
                }
            }
            ConfigAction::Status => {
                println!("Config path: {}", AppConfig::config_path().display());
                match &config.hf_token {
                    Some(t) if t.len() >= 8 => {
                        println!("HF Token: {}...{}", &t[..4], &t[t.len() - 4..]);
                    }
                    Some(_) => println!("HF Token: {}", "[set]".green()),
                    None => println!(
                        "HF Token: {} (use `llm-check config set-token <TOKEN>`)",
                        "Not set".yellow()
                    ),
                }
                match config.benchmark_bandwidth_gbps {
                    Some(bw) => println!("Stored Bandwidth: {:.2} GB/s", bw),
                    None => println!(
                        "Stored Bandwidth: {} (run `llm-check bench`)",
                        "Not benchmarked".yellow()
                    ),
                }
                match &config.carbon_region {
                    Some(r) => println!("Carbon Region: {}", r),
                    None => println!("Carbon Region: {} (default: US)", "Not set".yellow()),
                }
                match config.energy_cost_per_kwh {
                    Some(c) => println!("Energy Cost: ${:.2}/kWh", c),
                    None => println!("Energy Cost: {} (default: $0.12/kWh)", "Not set".yellow()),
                }
            }
        },
    }
}
