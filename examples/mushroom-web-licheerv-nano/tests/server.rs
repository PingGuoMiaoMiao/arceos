use core::convert::Infallible;

use arceos_mushroom_web_licheerv_nano::backend::InferenceBackendState;
use arceos_mushroom_web_licheerv_nano::envelope::{
    HEADER_LENGTH, MAGIC, PAYLOAD_LENGTH, TOTAL_LENGTH, VERSION,
};
use arceos_mushroom_web_licheerv_nano::server::{DuplexStream, ServeOutcome, serve_one};
use arceos_mushroom_web_licheerv_nano::service::{
    InferenceBackend, InferenceLock, ServiceReadiness,
};
use arceos_mushroom_web_licheerv_nano::stream::MAX_REQUEST_HEADER_LENGTH;
use axmodel_mushroom_yolov5::{InferenceResult, InferenceTiming, RgbChw640, crc32_ieee};

struct MemoryDuplex {
    input: Vec<u8>,
    position: usize,
    chunk_size: usize,
    output: Vec<u8>,
    now_micros: u64,
}

impl MemoryDuplex {
    fn new(input: Vec<u8>, chunk_size: usize) -> Self {
        Self {
            input,
            position: 0,
            chunk_size,
            output: Vec::new(),
            now_micros: 0,
        }
    }

    fn response(&self) -> &str {
        core::str::from_utf8(&self.output).unwrap()
    }
}

impl DuplexStream for MemoryDuplex {
    type ReadError = Infallible;
    type WriteError = Infallible;

    fn now_micros(&self) -> u64 {
        self.now_micros
    }

    fn read(&mut self, destination: &mut [u8]) -> Result<usize, Self::ReadError> {
        if self.position == self.input.len() {
            return Ok(0);
        }
        let count = destination
            .len()
            .min(self.chunk_size)
            .min(self.input.len() - self.position);
        destination[..count].copy_from_slice(&self.input[self.position..self.position + count]);
        self.position += count;
        self.now_micros += 5;
        Ok(count)
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::WriteError> {
        self.output.extend_from_slice(bytes);
        Ok(())
    }
}

#[derive(Default)]
struct RecordingBackend {
    calls: usize,
}

impl InferenceBackend for RecordingBackend {
    type Error = Infallible;

    fn infer(&mut self, _input: RgbChw640<'_>) -> Result<InferenceResult, Self::Error> {
        self.calls += 1;
        Ok(InferenceResult {
            detections: Vec::new(),
            timing: InferenceTiming {
                quantize_us: 1,
                tpu_us: 2,
                output_sync_us: 3,
                postprocess_us: 4,
                total_us: 10,
            },
        })
    }
}

fn valid_envelope() -> Vec<u8> {
    let mut bytes = vec![0_u8; TOTAL_LENGTH];
    bytes[..4].copy_from_slice(&MAGIC);
    bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
    bytes[6..8].copy_from_slice(&(HEADER_LENGTH as u16).to_le_bytes());
    bytes[8..12].copy_from_slice(&640_u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&480_u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&640_u32.to_le_bytes());
    bytes[20..24].copy_from_slice(&480_u32.to_le_bytes());
    bytes[24..28].copy_from_slice(&0_u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&80_u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&(PAYLOAD_LENGTH as u32).to_le_bytes());
    let crc = crc32_ieee(&bytes[HEADER_LENGTH..]);
    bytes[36..40].copy_from_slice(&crc.to_le_bytes());
    bytes
}

fn infer_request(body: &[u8]) -> Vec<u8> {
    let mut request = format!(
        "POST /api/infer HTTP/1.1\r\nContent-Type: application/vnd.arceos.rgb-u8\r\nContent-Length: {TOTAL_LENGTH}\r\n\r\n"
    )
    .into_bytes();
    request.extend_from_slice(body);
    request
}

fn serve(stream: &mut MemoryDuplex, backend: &mut RecordingBackend) -> ServeOutcome {
    let mut header = [0_u8; MAX_REQUEST_HEADER_LENGTH];
    let mut body = vec![0_u8; TOTAL_LENGTH];
    serve_one(
        stream,
        &mut header,
        &mut body,
        ServiceReadiness::READY,
        &InferenceLock::new(),
        backend,
    )
    .unwrap()
}

#[test]
fn health_request_returns_a_complete_http_response() {
    let mut stream = MemoryDuplex::new(b"GET /health HTTP/1.1\r\nHost: board\r\n\r\n".to_vec(), 3);
    let mut backend = RecordingBackend::default();

    assert_eq!(serve(&mut stream, &mut backend), ServeOutcome::Responded);
    assert_eq!(backend.calls, 0);
    assert!(stream.response().starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(stream.response().contains("\"wifi\":\"DhcpBound\""));
}

#[test]
fn valid_inference_request_calls_the_backend_once() {
    let mut stream = MemoryDuplex::new(infer_request(&valid_envelope()), 1536);
    let mut backend = RecordingBackend::default();

    let ServeOutcome::InferenceCompleted(report) = serve(&mut stream, &mut backend) else {
        panic!("expected inference completion report");
    };
    assert_eq!(backend.calls, 1);
    assert_eq!(report.request_id, 1);
    assert_eq!(
        report.input_crc32,
        crc32_ieee(&valid_envelope()[HEADER_LENGTH..])
    );
    assert_eq!(report.detection_count, 0);
    assert_eq!(report.receive_us, 4005);
    assert_eq!(report.timing.tpu_us, 2);
    assert!(stream.response().starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(stream.response().contains("\"request_id\":1"));
    assert!(stream.response().contains("\"receive\":4005"));
}

#[test]
fn malformed_http_writes_the_mapped_rejection_without_inference() {
    let mut stream = MemoryDuplex::new(b"BROKEN\r\n\r\n".to_vec(), 64);
    let mut backend = RecordingBackend::default();

    assert_eq!(serve(&mut stream, &mut backend), ServeOutcome::Responded);
    assert_eq!(backend.calls, 0);
    assert!(
        stream
            .response()
            .starts_with("HTTP/1.1 400 Bad Request\r\n")
    );
    assert!(stream.response().contains("\"code\":\"INVALID_HTTP\""));
}

#[test]
fn eof_during_headers_closes_without_inference_or_response() {
    let mut stream = MemoryDuplex::new(b"GET /health HTTP/1.1\r\nHost: board".to_vec(), 8);
    let mut backend = RecordingBackend::default();

    assert_eq!(serve(&mut stream, &mut backend), ServeOutcome::PeerClosed);
    assert_eq!(backend.calls, 0);
    assert!(stream.output.is_empty());
}

#[test]
fn eof_during_inference_body_closes_without_inference_or_response() {
    let envelope = valid_envelope();
    let mut stream = MemoryDuplex::new(infer_request(&envelope[..envelope.len() - 1]), 2048);
    let mut backend = RecordingBackend::default();

    assert_eq!(serve(&mut stream, &mut backend), ServeOutcome::PeerClosed);
    assert_eq!(backend.calls, 0);
    assert!(stream.output.is_empty());
}

#[test]
fn unavailable_engine_keeps_health_online_and_rejects_inference_with_503() {
    let mut backend = InferenceBackendState::<RecordingBackend>::unavailable();
    let readiness = backend.readiness(true);
    let lock = InferenceLock::new();
    let mut header = [0_u8; MAX_REQUEST_HEADER_LENGTH];
    let mut body = vec![0_u8; TOTAL_LENGTH];
    let mut health = MemoryDuplex::new(b"GET /health HTTP/1.1\r\nHost: board\r\n\r\n".to_vec(), 64);

    assert_eq!(
        serve_one(
            &mut health,
            &mut header,
            &mut body,
            readiness,
            &lock,
            &mut backend,
        )
        .unwrap(),
        ServeOutcome::Responded
    );
    assert!(health.response().starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(health.response().contains("\"tpu\":\"NotReady\""));

    let mut infer = MemoryDuplex::new(infer_request(&valid_envelope()), 4096);
    assert_eq!(
        serve_one(
            &mut infer,
            &mut header,
            &mut body,
            readiness,
            &lock,
            &mut backend,
        )
        .unwrap(),
        ServeOutcome::Responded
    );
    assert!(
        infer
            .response()
            .starts_with("HTTP/1.1 503 Service Unavailable\r\n")
    );
    assert!(infer.response().contains("\"code\":\"TPU_NOT_READY\""));
}
