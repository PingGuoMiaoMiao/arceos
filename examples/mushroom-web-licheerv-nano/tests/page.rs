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
