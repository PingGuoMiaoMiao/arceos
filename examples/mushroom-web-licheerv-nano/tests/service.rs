use core::convert::Infallible;

use axmodel_mushroom_yolov5::{Detection, InferenceResult, InferenceTiming, RgbChw640, crc32_ieee};
use mushroom_web_licheerv_nano::envelope::{
    HEADER_LENGTH, MAGIC, PAYLOAD_LENGTH, TOTAL_LENGTH, VERSION,
};
use mushroom_web_licheerv_nano::http::{RequestHead, RequestRoute};
use mushroom_web_licheerv_nano::response::ByteWriter;
use mushroom_web_licheerv_nano::service::{
    InferenceBackend, InferenceLock, ServiceReadiness, handle_request,
};

#[derive(Default)]
struct VecWriter(Vec<u8>);

impl ByteWriter for VecWriter {
    type Error = Infallible;

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

struct MockBackend {
    calls: usize,
    fail: bool,
}

impl InferenceBackend for MockBackend {
    type Error = ();

    fn infer(&mut self, input: RgbChw640<'_>) -> Result<InferenceResult, Self::Error> {
        self.calls += 1;
        assert_eq!(input.meta.source_width, 2560);
        assert_eq!(input.meta.source_height, 1440);
        assert_eq!(input.bytes.len(), PAYLOAD_LENGTH);
        if self.fail {
            return Err(());
        }
        Ok(InferenceResult {
            detections: vec![Detection {
                x1: 264.0,
                y1: 208.51,
                x2: 429.06,
                y2: 367.49,
                confidence: 0.924084,
            }],
            timing: InferenceTiming {
                quantize_us: 4100,
                tpu_us: 59_738,
                output_sync_us: 200,
                postprocess_us: 1800,
                total_us: 65_838,
            },
        })
    }
}

fn valid_envelope() -> Vec<u8> {
    let mut bytes = vec![0_u8; TOTAL_LENGTH];
    bytes[..4].copy_from_slice(&MAGIC);
    bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
    bytes[6..8].copy_from_slice(&(HEADER_LENGTH as u16).to_le_bytes());
    bytes[8..12].copy_from_slice(&2560_u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&1440_u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&640_u32.to_le_bytes());
    bytes[20..24].copy_from_slice(&360_u32.to_le_bytes());
    bytes[24..28].copy_from_slice(&0_u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&140_u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&(PAYLOAD_LENGTH as u32).to_le_bytes());
    for (index, byte) in bytes[HEADER_LENGTH..].iter_mut().enumerate() {
        *byte = (index & 0xff) as u8;
    }
    let crc = crc32_ieee(&bytes[HEADER_LENGTH..]);
    bytes[36..40].copy_from_slice(&crc.to_le_bytes());
    bytes
}

fn infer_head() -> RequestHead {
    RequestHead {
        route: RequestRoute::Infer,
        content_length: TOTAL_LENGTH,
        header_length: 0,
    }
}

fn body_text(writer: VecWriter) -> String {
    let text = String::from_utf8(writer.0).unwrap();
    text.split_once("\r\n\r\n").unwrap().1.to_owned()
}

#[test]
fn valid_inference_calls_the_backend_and_returns_the_fixed_contract() {
    let body = valid_envelope();
    let lock = InferenceLock::new();
    let mut backend = MockBackend {
        calls: 0,
        fail: false,
    };
    let mut writer = VecWriter::default();

    handle_request(
        infer_head(),
        &body,
        ServiceReadiness::READY,
        &lock,
        &mut backend,
        6000,
        &mut writer,
    )
    .unwrap();

    assert_eq!(backend.calls, 1);
    let body = body_text(writer);
    assert!(body.contains("\"request_id\":1"));
    assert!(body.contains("\"score\":0.924084"));
    assert!(body.contains("\"receive\":6000"));
}

#[test]
fn invalid_crc_never_calls_the_backend() {
    let mut body = valid_envelope();
    body[HEADER_LENGTH] ^= 1;
    let lock = InferenceLock::new();
    let mut backend = MockBackend {
        calls: 0,
        fail: false,
    };
    let mut writer = VecWriter::default();

    handle_request(
        infer_head(),
        &body,
        ServiceReadiness::READY,
        &lock,
        &mut backend,
        0,
        &mut writer,
    )
    .unwrap();

    assert_eq!(backend.calls, 0);
    let response = String::from_utf8(writer.0).unwrap();
    assert!(response.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    assert!(response.contains("\"code\":\"INVALID_ENVELOPE\""));
}

#[test]
fn held_inference_lock_returns_503_without_calling_the_backend() {
    let body = valid_envelope();
    let lock = InferenceLock::new();
    let _permit = lock.try_acquire().unwrap();
    let mut backend = MockBackend {
        calls: 0,
        fail: false,
    };
    let mut writer = VecWriter::default();

    handle_request(
        infer_head(),
        &body,
        ServiceReadiness::READY,
        &lock,
        &mut backend,
        0,
        &mut writer,
    )
    .unwrap();

    assert_eq!(backend.calls, 0);
    let response = String::from_utf8(writer.0).unwrap();
    assert!(response.starts_with("HTTP/1.1 503 Service Unavailable\r\n"));
    assert!(response.contains("\"code\":\"INFERENCE_BUSY\""));
}

#[test]
fn unavailable_tpu_returns_503_without_calling_the_backend() {
    let body = valid_envelope();
    let lock = InferenceLock::new();
    let mut backend = MockBackend {
        calls: 0,
        fail: false,
    };
    let mut writer = VecWriter::default();

    handle_request(
        infer_head(),
        &body,
        ServiceReadiness {
            wifi_ready: true,
            tpu_ready: false,
        },
        &lock,
        &mut backend,
        0,
        &mut writer,
    )
    .unwrap();

    assert_eq!(backend.calls, 0);
    let response = String::from_utf8(writer.0).unwrap();
    assert!(response.starts_with("HTTP/1.1 503 Service Unavailable\r\n"));
    assert!(response.contains("\"code\":\"TPU_NOT_READY\""));
}

#[test]
fn backend_failure_becomes_500_and_releases_the_lock() {
    let body = valid_envelope();
    let lock = InferenceLock::new();
    let mut backend = MockBackend {
        calls: 0,
        fail: true,
    };
    let mut writer = VecWriter::default();

    handle_request(
        infer_head(),
        &body,
        ServiceReadiness::READY,
        &lock,
        &mut backend,
        0,
        &mut writer,
    )
    .unwrap();

    assert_eq!(backend.calls, 1);
    assert!(lock.try_acquire().is_ok());
    let response = String::from_utf8(writer.0).unwrap();
    assert!(response.starts_with("HTTP/1.1 500 Internal Server Error\r\n"));
    assert!(response.contains("\"code\":\"TPU_EXECUTION_FAILED\""));
}
