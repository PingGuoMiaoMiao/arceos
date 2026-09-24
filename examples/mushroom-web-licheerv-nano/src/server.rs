use crate::http::HttpRejection;
use crate::response::ByteWriter;
use crate::service::{
    InferenceBackend, InferenceLock, InferenceReport, ServiceOutcome, ServiceReadiness,
    handle_request, write_http_rejection,
};
use crate::stream::{ByteReader, MAX_REQUEST_HEADER_LENGTH, ReceiveError, receive_request};

pub trait DuplexStream {
    type ReadError;
    type WriteError;

    fn now_micros(&self) -> u64;
    fn read(&mut self, destination: &mut [u8]) -> Result<usize, Self::ReadError>;
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::WriteError>;
}

impl<T: DuplexStream> ByteReader for T {
    type Error = T::ReadError;

    fn read(&mut self, destination: &mut [u8]) -> Result<usize, Self::Error> {
        DuplexStream::read(self, destination)
    }
}

impl<T: DuplexStream> ByteWriter for T {
    type Error = T::WriteError;

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        DuplexStream::write_all(self, bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServeOutcome {
    Responded,
    InferenceCompleted(InferenceReport),
    PeerClosed,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ServeError<R, W> {
    Read(R),
    Write(W),
}

pub fn serve_one<S, B>(
    stream: &mut S,
    header_storage: &mut [u8; MAX_REQUEST_HEADER_LENGTH],
    body_storage: &mut [u8],
    readiness: ServiceReadiness,
    inference_lock: &InferenceLock,
    backend: &mut B,
) -> Result<ServeOutcome, ServeError<S::ReadError, S::WriteError>>
where
    S: DuplexStream,
    B: InferenceBackend,
{
    let receive_started_us = stream.now_micros();
    let request = match receive_request(stream, header_storage, body_storage) {
        Ok(request) => request,
        Err(ReceiveError::Read(error)) => return Err(ServeError::Read(error)),
        Err(ReceiveError::UnexpectedEof) => return Ok(ServeOutcome::PeerClosed),
        Err(ReceiveError::HeaderTooLarge) => {
            write_http_rejection(
                stream,
                HttpRejection {
                    status: 413,
                    code: "HEADER_TOO_LARGE",
                },
            )
            .map_err(ServeError::Write)?;
            return Ok(ServeOutcome::Responded);
        }
        Err(ReceiveError::Http(rejection)) => {
            write_http_rejection(stream, rejection).map_err(ServeError::Write)?;
            return Ok(ServeOutcome::Responded);
        }
    };
    let receive_us = stream.now_micros().saturating_sub(receive_started_us);

    let outcome = handle_request(
        request.head,
        request.body,
        readiness,
        inference_lock,
        backend,
        receive_us,
        stream,
    )
    .map_err(ServeError::Write)?;
    Ok(match outcome {
        ServiceOutcome::Responded => ServeOutcome::Responded,
        ServiceOutcome::InferenceCompleted(report) => ServeOutcome::InferenceCompleted(report),
    })
}
