use serde_json::Value;
use std::time::Duration;

pub struct DeepseekProvider;

impl DeepseekProvider {
    pub async fn extract(
        prompt: &str,
        api_key: &str,
        image_data: Option<(&str, &str)>, // (mime_type, base64_data)
    ) -> Option<Value> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .build()
            .ok()?;

        let body = if let Some((mime_type, data)) = image_data {
            tracing::info!("SmartInput: Chamando modelo DeepSeek 'deepseek-v4-flash-vision-exp' com imagem...");
            serde_json::json!({
                "model": "deepseek-v4-flash-vision-exp",
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            { "type": "text", "text": prompt },
                            { "type": "image_url", "image_url": { "url": format!("data:{};base64,{}", mime_type, data) } }
                        ]
                    }
                ],
                "response_format": { "type": "json_object" },
                "temperature": 0.0
            })
        } else {
            tracing::info!("SmartInput: Chamando modelo DeepSeek 'deepseek-chat'...");
            serde_json::json!({
                "model": "deepseek-chat",
                "messages": [
                    { "role": "user", "content": prompt }
                ],
                "response_format": { "type": "json_object" },
                "temperature": 0.0
            })
        };

        let res = client
            .post("https://api.deepseek.com/chat/completions")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await;

        let res = match res {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                tracing::warn!("DeepSeek API failed with status {}: {}", status, text);
                return None;
            }
            Err(e) => {
                tracing::error!("DeepSeek API error: {}", e);
                return None;
            }
        };

        let json_resp: Value = res.json().await.ok()?;
        let raw_text = json_resp
            .get("choices")?
            .get(0)?
            .get("message")?
            .get("content")?
            .as_str()?;

        clean_json_response(raw_text)
    }
}

pub fn clean_json_response(raw_text: &str) -> Option<Value> {
    let mut clean = raw_text.trim();
    if clean.starts_with("```json") {
        clean = &clean[7..];
    } else if clean.starts_with("```") {
        clean = &clean[3..];
    }
    if clean.ends_with("```") {
        clean = &clean[..clean.len() - 3];
    }
    let clean = clean.trim();

    serde_json::from_str(clean).map_err(|e| {
        tracing::error!("Failed to parse JSON response: {}. Content: {}", e, clean);
    }).ok()
}
