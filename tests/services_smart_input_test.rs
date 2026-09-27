use hrm_api_v2::services::smart_input::deepseek::clean_json_response;

#[test]
fn test_clean_json_response_with_markdown_blocks() {
    let raw_with_markdown = r#"```json
    {
      "companies": [{ "name": "Agência Prime" }],
      "expenses": [{ "amount": 120.0, "description": "Almoço" }]
    }
    ```"#;

    let res = clean_json_response(raw_with_markdown).expect("Failed to clean json");
    assert_eq!(res["companies"][0]["name"], "Agência Prime");
    assert_eq!(res["expenses"][0]["amount"], 120.0);
}

#[test]
fn test_clean_json_response_plain() {
    let raw_plain = r#"{"status": "ok", "count": 5}"#;
    let res = clean_json_response(raw_plain).expect("Failed to parse plain json");
    assert_eq!(res["status"], "ok");
    assert_eq!(res["count"], 5);
}

#[test]
fn test_clean_json_invalid() {
    let raw_invalid = "This is not JSON";
    let res = clean_json_response(raw_invalid);
    assert!(res.is_none());
}
