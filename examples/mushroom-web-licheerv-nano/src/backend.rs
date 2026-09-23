use axmodel_mushroom_yolov5::{InferenceResult, RgbChw640};

use crate::service::{InferenceBackend, ServiceReadiness};

pub enum InferenceBackendState<B> {
    Ready(B),
    Unavailable,
}

impl<B> InferenceBackendState<B> {
    pub const fn ready(backend: B) -> Self {
        Self::Ready(backend)
    }

    pub const fn unavailable() -> Self {
        Self::Unavailable
    }

    pub const fn readiness(&self, wifi_ready: bool) -> ServiceReadiness {
        ServiceReadiness {
            wifi_ready,
            tpu_ready: matches!(self, Self::Ready(_)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferenceBackendStateError<E> {
    Backend(E),
    Unavailable,
}

impl<B: InferenceBackend> InferenceBackend for InferenceBackendState<B> {
    type Error = InferenceBackendStateError<B::Error>;

    fn infer(&mut self, input: RgbChw640<'_>) -> Result<InferenceResult, Self::Error> {
        match self {
            Self::Ready(backend) => backend
                .infer(input)
                .map_err(InferenceBackendStateError::Backend),
            Self::Unavailable => Err(InferenceBackendStateError::Unavailable),
        }
    }
}
