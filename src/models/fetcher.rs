// src/models/fetcher.rs
use crate::models::{ModelMetadata, Suggestion};
use regex::Regex;

pub async fn fetch_metadata(
    model_id: &str,
    token: Option<&str>,
) -> Result<ModelMetadata, Box<dyn std::error::Error>> {
    let url = format!(
        "https://huggingface.co/{}/resolve/main/config.json",
        model_id
    );

    let client = reqwest::Client::builder()
        .user_agent("llm-check/1.0")
        .build()?;

    let mut request = client.get(url);

    if let Some(t) = token {
        request = request.bearer_auth(t);
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        return Err(format!("HF Error {}: Model gated or not found.", response.status()).into());
    }

    let metadata: ModelMetadata = response.json().await?;
    Ok(metadata)
}

pub async fn discover_models(
    token: Option<&str>,
) -> Result<Vec<Suggestion>, Box<dyn std::error::Error>> {
    let url = "https://huggingface.co/api/models?library=gguf&pipeline_tag=text-generation&sort=downloads&limit=20";

    let client = reqwest::Client::builder()
        .user_agent("llm-check/1.0")
        .build()?;

    let mut request = client.get(url);

    if let Some(t) = token {
        request = request.bearer_auth(t);
    }

    let response = request.send().await?;
    let models: Vec<serde_json::Value> = response.json().await?;

    let mut suggestions = Vec::new();
    let re = Regex::new(r"([0-9.]+)").unwrap(); // Regex to find "8B", "70B", etc.

    for m in models {
        let id = m["id"].as_str().unwrap_or("");

        // Use Regex to extract the parameter count from the name (e.g., "8B" -> 8.0)
        if let Some(caps) = re.captures(id) {
            let p_str: String = caps.get(1).unwrap().as_str().to_uppercase();
            let params: f64 = p_str.replace('B', "").parse().unwrap_or(0.0);

            suggestions.push(Suggestion {
                model_id: id.to_string(),
                name: id.split('/').next_back().unwrap_or(id).to_string(),
                params_bn: params,
            });
        }
    }
    Ok(suggestions)
}
