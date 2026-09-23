use mushroom_web_licheerv_nano::page::INDEX_HTML;

#[test]
fn embedded_page_contains_the_fixed_upload_contract() {
    let html = core::str::from_utf8(INDEX_HTML).unwrap();
    for required in [
        "PhoneImageEnvelopeV1",
        "application/vnd.arceos.rgb-u8",
        "/api/infer",
        "/health",
        "ARIM",
        "1228800",
    ] {
        assert!(html.contains(required), "missing page marker: {required}");
    }
}

#[test]
fn embedded_page_has_no_external_runtime_dependency() {
    let html = core::str::from_utf8(INDEX_HTML).unwrap();
    assert!(!html.contains("<script src="));
    assert!(!html.contains("<link rel=\"stylesheet\""));
}

#[test]
fn embedded_page_uses_the_same_integer_letterbox_rules_as_rust() {
    let html = core::str::from_utf8(INDEX_HTML).unwrap();
    assert!(html.contains("Math.floor((height * SIZE) / width)"));
    assert!(html.contains("Math.floor((width * SIZE) / height)"));
    assert!(!html.contains("Math.floor(height * (SIZE / width))"));
}

#[test]
fn embedded_page_locks_photo_selection_during_inference() {
    let html = core::str::from_utf8(INDEX_HTML).unwrap();
    assert!(html.contains("photo.disabled = true"));
    assert!(html.contains("photo.disabled = false"));
    assert!(html.contains("const inferenceBitmap = selectedBitmap"));
    assert!(html.contains("drawResult(inferenceBitmap, detections)"));
}
