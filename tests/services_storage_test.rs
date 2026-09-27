use base64::{engine::general_purpose::STANDARD, Engine as _};

#[test]
fn test_storage_key_generation() {
    let raw = b"test content for attachment";
    let digest = md5::compute(raw);
    let checksum = STANDARD.encode(digest.0);
    assert!(!checksum.is_empty());
}

#[test]
fn test_base64_data_uri_cleaning() {
    let data_uri = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY44YAAAAASUVORK5CYII=";
    
    assert!(data_uri.starts_with("data:"));
    let idx = data_uri.find(";base64,").unwrap();
    let mime = &data_uri[5..idx];
    let b64 = &data_uri[idx + 8..];

    assert_eq!(mime, "image/png");
    let decoded = STANDARD.decode(b64).unwrap();
    assert!(!decoded.is_empty());
}
