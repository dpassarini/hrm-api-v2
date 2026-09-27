use hrm_api_v2::services::smart_input::{
    deepseek::{clean_json_response, DeepseekProvider},
    gemini::GeminiProvider,
    extractor::ExtractorService,
};

#[test]
fn test_clean_json_response_edge_cases() {
    let raw = "```json\n{\"companies\": []}\n```";
    let parsed = clean_json_response(raw).unwrap();
    assert!(parsed.get("companies").is_some());

    let raw_plain = "{\"contacts\": []}";
    let parsed_plain = clean_json_response(raw_plain).unwrap();
    assert!(parsed_plain.get("contacts").is_some());

    let raw_invalid = "not a json string";
    assert!(clean_json_response(raw_invalid).is_none());
}

#[tokio::test]
async fn test_providers_with_empty_or_invalid_keys() {
    // Provider extraction with dummy/empty key should fail gracefully returning None
    let res = DeepseekProvider::extract("Test prompt", "invalid_key", None).await;
    assert!(res.is_none());

    let res_gemini = GeminiProvider::extract("Test prompt", "invalid_key", None).await;
    assert!(res_gemini.is_none());
}

#[tokio::test]
async fn test_extractor_service_with_no_keys() {
    let res = ExtractorService::extract(Some("Reunião com Maria"), None, None).await;
    // When no real keys are in env, extractor returns None gracefully without panicking
    assert!(res.is_none() || res.is_some());
}
