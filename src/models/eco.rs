// src/models/eco.rs
use serde::Serialize;

/// Regional carbon intensity defaults (g CO2 per kWh)
pub fn carbon_intensity_for_region(region: &str) -> f64 {
    match region.to_uppercase().as_str() {
        "US" => 385.0,
        "EU" => 320.0,
        "UK" => 230.0,
        "DE" => 350.0, // Germany
        "FR" => 55.0,  // France (nuclear)
        "NO" => 20.0,  // Norway (hydro)
        "CN" => 580.0, // China
        "IN" => 700.0, // India
        "JP" => 450.0, // Japan
        "AU" => 650.0, // Australia
        "CA" => 120.0, // Canada (hydro-heavy)
        "BR" => 75.0,  // Brazil (hydro)
        "GLOBAL" => 440.0,
        _ => 385.0, // Default to US
    }
}

/// List of supported regions for display
pub const SUPPORTED_REGIONS: &[(&str, &str)] = &[
    ("US", "United States (385 g/kWh)"),
    ("EU", "European Union (320 g/kWh)"),
    ("UK", "United Kingdom (230 g/kWh)"),
    ("DE", "Germany (350 g/kWh)"),
    ("FR", "France (55 g/kWh)"),
    ("NO", "Norway (20 g/kWh)"),
    ("CN", "China (580 g/kWh)"),
    ("IN", "India (700 g/kWh)"),
    ("JP", "Japan (450 g/kWh)"),
    ("AU", "Australia (650 g/kWh)"),
    ("CA", "Canada (120 g/kWh)"),
    ("BR", "Brazil (75 g/kWh)"),
    ("GLOBAL", "Global average (440 g/kWh)"),
];

#[derive(Debug, Serialize)]
pub struct EcoReport {
    pub tdp_watts: f64,
    pub tokens_per_sec: f64,
    pub joules_per_token: f64,
    pub kwh_per_1k_tokens: f64,
    pub co2_grams_per_1k_tokens: f64,
    pub cost_usd_per_1k_tokens: f64,
    pub carbon_intensity_g_per_kwh: f64,
    pub energy_cost_per_kwh: f64,
    pub region: String,
}

/// Calculate eco metrics for a given inference scenario.
///
/// - `tdp_watts`: GPU/chip power draw under load
/// - `tps`: predicted tokens per second
/// - `region`: carbon intensity region code
/// - `carbon_override`: optional manual carbon intensity (g CO2/kWh)
/// - `cost_per_kwh`: electricity cost in USD/kWh
pub fn calculate_eco(
    tdp_watts: f64,
    tps: f64,
    region: &str,
    carbon_override: Option<f64>,
    cost_per_kwh: Option<f64>,
) -> EcoReport {
    let carbon_intensity = carbon_override.unwrap_or_else(|| carbon_intensity_for_region(region));
    let energy_cost = cost_per_kwh.unwrap_or(0.12); // US average default

    // Joules per token = watts / (tokens/sec) = joules/token
    let joules_per_token = tdp_watts / tps;

    // kWh per 1000 tokens = (joules_per_token * 1000) / 3_600_000
    let kwh_per_1k = (joules_per_token * 1000.0) / 3_600_000.0;

    // CO2 grams per 1000 tokens
    let co2_per_1k = kwh_per_1k * carbon_intensity;

    // Cost per 1000 tokens in USD
    let cost_per_1k = kwh_per_1k * energy_cost;

    EcoReport {
        tdp_watts,
        tokens_per_sec: tps,
        joules_per_token,
        kwh_per_1k_tokens: kwh_per_1k,
        co2_grams_per_1k_tokens: co2_per_1k,
        cost_usd_per_1k_tokens: cost_per_1k,
        carbon_intensity_g_per_kwh: carbon_intensity,
        energy_cost_per_kwh: energy_cost,
        region: region.to_uppercase(),
    }
}
