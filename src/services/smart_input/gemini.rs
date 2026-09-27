use serde_json::Value;
use std::time::Duration;

use super::deepseek::clean_json_response;

pub struct GeminiProvider;

impl GeminiProvider {
    pub async fn extract(
        prompt: &str,
        api_key: &str,
        image_data: Option<(&str, &str)>, // (mime_type, base64_data)
    ) -> Option<Value> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .build()
            .ok()?;

        let mut parts = Vec::new();

        if let Some((mime_type, data)) = image_data {
            parts.push(serde_json::json!({
                "inline_data": {
                    "mime_type": mime_type,
                    "data": data
                }
            }));
        }

        parts.push(serde_json::json!({
            "text": prompt
        }));

        let body = serde_json::json!({
            "contents": [
                {
                    "parts": parts
                }
            ]
        });

        let models_to_try = [
            "gemini-3.7-flash",
            "gemini-3.5-flash",
            "gemini-2.5-flash",
            "gemini-2.5-pro",
        ];

        for (index, model_name) in models_to_try.iter().enumerate() {
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                model_name, api_key
            );

            tracing::info!("SmartInput: Chamando modelo Gemini '{}'...", model_name);

            let res = client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            match res {
                Ok(r) if r.status().is_success() => {
                    let json_resp: Value = match r.json().await {
                        Ok(j) => j,
                        Err(e) => {
                            tracing::error!("Failed to parse Gemini response JSON: {}", e);
                            continue;
                        }
                    };

                    let raw_text = json_resp
                        .get("candidates")
                        .and_then(|c| c.get(0))
                        .and_then(|c0| c0.get("content"))
                        .and_then(|cnt| cnt.get("parts"))
                        .and_then(|p| p.get(0))
                        .and_then(|p0| p0.get("text"))
                        .and_then(|t| t.as_str());

                    if let Some(text) = raw_text {
                        if let Some(val) = clean_json_response(text) {
                            return Some(val);
                        }
                    }
                }
                Ok(r) if r.status().as_u16() == 503 => {
                    tracing::warn!("Aviso: O modelo Gemini '{}' está sobrecarregado (Erro 503).", model_name);
                    if index < models_to_try.len() - 1 {
                        tracing::warn!("Fallback acionado: Tentando utilizar o próximo modelo.");
                        continue;
                    }
                }
                Ok(r) => {
                    let status = r.status();
                    let text = r.text().await.unwrap_or_default();
                    tracing::error!("Gemini API failed with status {}: {}", status, text);
                    if index < models_to_try.len() - 1 {
                        continue;
                    }
                }
                Err(e) => {
                    tracing::error!("Gemini API request error: {}", e);
                    if index < models_to_try.len() - 1 {
                        continue;
                    }
                }
            }
        }

        None
    }
}
