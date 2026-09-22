use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use axmodel_mushroom_yolov5::{
    InferenceResult, InferenceTiming, MODEL_INPUT_LENGTH, MODEL_NAME, RgbChw640,
};

use crate::envelope::PhoneImageEnvelopeV1;
use crate::http::{HttpRejection, RequestHead, RequestRoute};
use crate::response::{
    ByteWriter, HealthReport, InferenceJson, write_error_response, write_health_response,
    write_index_response, write_inference_response,
};

pub trait InferenceBackend {
    type Error;

    fn infer(&mut self, input: RgbChw640<'_>) -> Result<InferenceResult, Self::Error>;
}

#[cfg(feature = "hardware")]
impl InferenceBackend for axmodel_mushroom_yolov5::engine::TpuEngine {
    type Error = axmodel_mushroom_yolov5::engine::TpuEngineError;

    fn infer(&mut self, input: RgbChw640<'_>) -> Result<InferenceResult, Self::Error> {
        axmodel_mushroom_yolov5::engine::TpuEngine::infer(self, input)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceReadiness {
    pub wifi_ready: bool,
    pub tpu_ready: bool,
}

impl ServiceReadiness {
    pub const READY: Self = Self {
        wifi_ready: true,
        tpu_ready: true,
    };
}

pub struct InferenceLock {
    held: AtomicBool,
    next_request_id: AtomicU64,
}

impl InferenceLock {
    pub const fn new() -> Self {
        Self {
            held: AtomicBool::new(false),
            next_request_id: AtomicU64::new(1),
        }
    }

    pub fn try_acquire(&self) -> Result<InferencePermit<'_>, InferenceBusy> {
        self.held
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map_err(|_| InferenceBusy)?;
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        Ok(InferencePermit {
            lock: self,
            request_id,
        })
    }
}

impl Default for InferenceLock {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InferenceBusy;

pub struct InferencePermit<'a> {
    lock: &'a InferenceLock,
    request_id: u64,
}

impl InferencePermit<'_> {
    pub const fn request_id(&self) -> u64 {
        self.request_id
    }
}

impl Drop for InferencePermit<'_> {
    fn drop(&mut self) {
        self.lock.held.store(false, Ordering::Release);
    }
}

pub fn handle_request<B, W>(
    head: RequestHead,
    body: &[u8],
    readiness: ServiceReadiness,
    inference_lock: &InferenceLock,
    backend: &mut B,
    receive_us: u64,
    writer: &mut W,
) -> Result<(), W::Error>
where
    B: InferenceBackend,
    W: ByteWriter,
{
    match head.route {
        RequestRoute::Index => write_index_response(writer),
        RequestRoute::Health => write_health_response(
            writer,
            HealthReport {
                wifi: if readiness.wifi_ready {
                    "DhcpBound"
                } else {
                    "NotReady"
                },
                tpu: if readiness.tpu_ready {
                    "Ready"
                } else {
                    "NotReady"
                },
                model: MODEL_NAME,
            },
        ),
        RequestRoute::Infer => {
            handle_inference(body, readiness, inference_lock, backend, receive_us, writer)
        }
    }
}

pub fn write_http_rejection<W: ByteWriter>(
    writer: &mut W,
    rejection: HttpRejection,
) -> Result<(), W::Error> {
    write_error_response(writer, rejection.status, rejection.code, rejection.code)
}

fn handle_inference<B, W>(
    body: &[u8],
    readiness: ServiceReadiness,
    inference_lock: &InferenceLock,
    backend: &mut B,
    receive_us: u64,
    writer: &mut W,
) -> Result<(), W::Error>
where
    B: InferenceBackend,
    W: ByteWriter,
{
    if !readiness.wifi_ready {
        return write_error_response(writer, 503, "WIFI_NOT_READY", "Wi-Fi is not ready");
    }
    if !readiness.tpu_ready {
        return write_error_response(writer, 503, "TPU_NOT_READY", "TPU is not ready");
    }

    let envelope = match PhoneImageEnvelopeV1::parse(body) {
        Ok(envelope) => envelope,
        Err(_) => {
            return write_error_response(
                writer,
                400,
                "INVALID_ENVELOPE",
                "PhoneImageEnvelopeV1 validation failed",
            );
        }
    };
    let payload: &[u8; MODEL_INPUT_LENGTH] = match envelope.payload.try_into() {
        Ok(payload) => payload,
        Err(_) => {
            return write_error_response(
                writer,
                400,
                "INVALID_ENVELOPE",
                "PhoneImageEnvelopeV1 payload length is invalid",
            );
        }
    };
    let permit = match inference_lock.try_acquire() {
        Ok(permit) => permit,
        Err(_) => {
            return write_error_response(
                writer,
                503,
                "INFERENCE_BUSY",
                "Another inference request is running",
            );
        }
    };

    let inference = match backend.infer(RgbChw640 {
        meta: envelope.meta,
        bytes: payload,
    }) {
        Ok(inference) => inference,
        Err(_) => {
            return write_error_response(
                writer,
                500,
                "TPU_EXECUTION_FAILED",
                "TPU inference failed",
            );
        }
    };
    let response_timing = InferenceTiming {
        total_us: receive_us.saturating_add(inference.timing.total_us),
        ..inference.timing
    };
    write_inference_response(
        writer,
        InferenceJson {
            request_id: permit.request_id(),
            model: MODEL_NAME,
            source_width: envelope.meta.source_width,
            source_height: envelope.meta.source_height,
            detections: &inference.detections,
            receive_us,
            timing: response_timing,
        },
    )
}
