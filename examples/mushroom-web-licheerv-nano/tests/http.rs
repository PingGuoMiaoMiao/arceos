use arceos_mushroom_web_licheerv_nano::envelope::TOTAL_LENGTH;
use arceos_mushroom_web_licheerv_nano::http::{
    HttpRejection, INFERENCE_CONTENT_TYPE, RequestRoute, parse_request_head,
};

fn rejection(status: u16, code: &'static str) -> HttpRejection {
    HttpRejection { status, code }
}

#[test]
fn routes_the_three_fixed_interfaces() {
    assert_eq!(
        parse_request_head(b"GET / HTTP/1.1\r\nHost: board\r\n\r\n")
            .unwrap()
            .route,
        RequestRoute::Index
    );
    assert_eq!(
        parse_request_head(b"GET /health HTTP/1.1\r\nHost: board\r\n\r\n")
            .unwrap()
            .route,
        RequestRoute::Health
    );
    let request = format!(
        "POST /api/infer HTTP/1.1\r\nHost: board\r\nContent-Type: {INFERENCE_CONTENT_TYPE}\r\nContent-Length: {TOTAL_LENGTH}\r\n\r\n"
    );
    let parsed = parse_request_head(request.as_bytes()).unwrap();
    assert_eq!(parsed.route, RequestRoute::Infer);
    assert_eq!(parsed.content_length, TOTAL_LENGTH);
}

#[test]
fn header_names_are_ascii_case_insensitive() {
    let request = format!(
        "POST /api/infer HTTP/1.1\r\ncontent-type: {INFERENCE_CONTENT_TYPE}\r\ncontent-length: {TOTAL_LENGTH}\r\n\r\n"
    );
    assert_eq!(
        parse_request_head(request.as_bytes()).unwrap().route,
        RequestRoute::Infer
    );
}

#[test]
fn unknown_paths_and_wrong_methods_have_distinct_statuses() {
    assert_eq!(
        parse_request_head(b"GET /missing HTTP/1.1\r\n\r\n"),
        Err(rejection(404, "NOT_FOUND"))
    );
    assert_eq!(
        parse_request_head(b"POST /health HTTP/1.1\r\n\r\n"),
        Err(rejection(405, "METHOD_NOT_ALLOWED"))
    );
}

#[test]
fn infer_requires_content_length() {
    let request =
        format!("POST /api/infer HTTP/1.1\r\nContent-Type: {INFERENCE_CONTENT_TYPE}\r\n\r\n");
    assert_eq!(
        parse_request_head(request.as_bytes()),
        Err(rejection(411, "LENGTH_REQUIRED"))
    );
}

#[test]
fn infer_rejects_oversized_and_short_bodies_before_receiving_them() {
    let oversized = format!(
        "POST /api/infer HTTP/1.1\r\nContent-Type: {INFERENCE_CONTENT_TYPE}\r\nContent-Length: {}\r\n\r\n",
        TOTAL_LENGTH + 1
    );
    assert_eq!(
        parse_request_head(oversized.as_bytes()),
        Err(rejection(413, "PAYLOAD_TOO_LARGE"))
    );

    let short = format!(
        "POST /api/infer HTTP/1.1\r\nContent-Type: {INFERENCE_CONTENT_TYPE}\r\nContent-Length: {}\r\n\r\n",
        TOTAL_LENGTH - 1
    );
    assert_eq!(
        parse_request_head(short.as_bytes()),
        Err(rejection(400, "INVALID_LENGTH"))
    );
}

#[test]
fn infer_requires_the_fixed_media_type() {
    let request = format!(
        "POST /api/infer HTTP/1.1\r\nContent-Type: application/octet-stream\r\nContent-Length: {TOTAL_LENGTH}\r\n\r\n"
    );
    assert_eq!(
        parse_request_head(request.as_bytes()),
        Err(rejection(415, "UNSUPPORTED_MEDIA_TYPE"))
    );
}

#[test]
fn rejects_missing_header_terminator_and_duplicate_lengths() {
    assert_eq!(
        parse_request_head(b"GET / HTTP/1.1\r\nHost: board"),
        Err(rejection(400, "INVALID_HTTP"))
    );
    let request = format!(
        "POST /api/infer HTTP/1.1\r\nContent-Type: {INFERENCE_CONTENT_TYPE}\r\nContent-Length: {TOTAL_LENGTH}\r\nContent-Length: {TOTAL_LENGTH}\r\n\r\n"
    );
    assert_eq!(
        parse_request_head(request.as_bytes()),
        Err(rejection(400, "INVALID_HTTP"))
    );
}
