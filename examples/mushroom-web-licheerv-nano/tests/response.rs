use axmodel_mushroom_yolov5::{Detection, InferenceTiming};
use mushroom_web_licheerv_nano::response::{
    ByteWriter, HealthReport, InferenceJson, write_error_response, write_health_response,
    write_inference_response,
};

#[derive(Default)]
struct VecWriter(Vec<u8>);

impl ByteWriter for VecWriter {
    type Error = ();

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

fn response_parts(writer: VecWriter) -> (String, String) {
    let response = String::from_utf8(writer.0).unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    let content_length = head
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert_eq!(content_length, body.len());
    (head.to_owned(), body.to_owned())
}

#[test]
fn health_response_is_json_and_has_an_exact_content_length() {
    let mut writer = VecWriter::default();
    write_health_response(
        &mut writer,
        HealthReport {
            wifi: "DhcpBound",
            tpu: "Ready",
            model: "mushroom_yolov5s_cv181x_int8_sym",
        },
    )
    .unwrap();

    let (head, body) = response_parts(writer);
    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(head.contains("Content-Type: application/json; charset=utf-8"));
    assert_eq!(
        body,
        "{\"wifi\":\"DhcpBound\",\"tpu\":\"Ready\",\"model\":\"mushroom_yolov5s_cv181x_int8_sym\"}"
    );
}

#[test]
fn error_response_escapes_json_control_characters() {
    let mut writer = VecWriter::default();
    write_error_response(&mut writer, 400, "INVALID_ENVELOPE", "bad \"magic\"\nline").unwrap();

    let (head, body) = response_parts(writer);
    assert!(head.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    assert_eq!(
        body,
        "{\"error\":{\"code\":\"INVALID_ENVELOPE\",\"message\":\"bad \\\"magic\\\"\\nline\"}}"
    );
}

#[test]
fn inference_response_matches_the_browser_contract() {
    let detections = [Detection {
        x1: 10.0,
        y1: 20.5,
        x2: 110.0,
        y2: 220.25,
        confidence: 0.995,
    }];
    let mut writer = VecWriter::default();
    write_inference_response(
        &mut writer,
        InferenceJson {
            request_id: 7,
            model: "mushroom_yolov5s_cv181x_int8_sym",
            source_width: 2560,
            source_height: 1440,
            detections: &detections,
            receive_us: 6000,
            timing: InferenceTiming {
                quantize_us: 4000,
                tpu_us: 59_738,
                output_sync_us: 200,
                postprocess_us: 1800,
                total_us: 65_738,
            },
        },
    )
    .unwrap();

    let (head, body) = response_parts(writer);
    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(body.contains("\"request_id\":7"));
    assert!(body.contains("\"model\":\"mushroom_yolov5s_cv181x_int8_sym\""));
    assert!(body.contains("\"image\":{\"width\":2560,\"height\":1440}"));
    assert!(body.contains("\"label\":\"mushroom\""));
    assert!(body.contains("\"score\":0.995000"));
    assert!(body.contains("\"x1\":10.000000"));
    assert!(body.contains("\"y1\":20.500000"));
    assert!(body.contains(
        "\"timing_us\":{\"receive\":6000,\"quantize\":4000,\"tpu\":59738,\"postprocess\":1800,\"total\":65738}"
    ));
}
