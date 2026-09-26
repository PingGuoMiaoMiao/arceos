use crate::http::{HttpRejection, RequestHead, parse_request_head};

pub const MAX_REQUEST_HEADER_LENGTH: usize = 4096;

pub trait ByteReader {
    type Error;

    fn read(&mut self, destination: &mut [u8]) -> Result<usize, Self::Error>;
}

#[derive(Debug, Eq, PartialEq)]
pub enum ReceiveError<E> {
    Read(E),
    UnexpectedEof,
    HeaderTooLarge,
    Http(HttpRejection),
}

#[derive(Debug, Eq, PartialEq)]
pub struct ReceivedRequest<'a> {
    pub head: RequestHead,
    pub body: &'a [u8],
}

pub fn receive_request<'a, R: ByteReader>(
    reader: &mut R,
    header_storage: &mut [u8; MAX_REQUEST_HEADER_LENGTH],
    body_storage: &'a mut [u8],
) -> Result<ReceivedRequest<'a>, ReceiveError<R::Error>> {
    let mut header_filled = 0;
    let head = loop {
        if let Some(terminator) = find_header_terminator(&header_storage[..header_filled]) {
            let header_length = terminator + 4;
            let head =
                parse_request_head(&header_storage[..header_length]).map_err(ReceiveError::Http)?;
            break head;
        }
        if header_filled == header_storage.len() {
            return Err(ReceiveError::HeaderTooLarge);
        }
        let count = reader
            .read(&mut header_storage[header_filled..])
            .map_err(ReceiveError::Read)?;
        if count == 0 {
            return Err(ReceiveError::UnexpectedEof);
        }
        header_filled += count;
    };

    if body_storage.len() < head.content_length {
        return Err(ReceiveError::Http(HttpRejection {
            status: 413,
            code: "PAYLOAD_TOO_LARGE",
        }));
    }

    let already_received = header_filled - head.header_length;
    if already_received > head.content_length {
        return Err(ReceiveError::Http(HttpRejection {
            status: 400,
            code: "INVALID_HTTP",
        }));
    }
    body_storage[..already_received]
        .copy_from_slice(&header_storage[head.header_length..header_filled]);

    let mut body_filled = already_received;
    while body_filled < head.content_length {
        let count = reader
            .read(&mut body_storage[body_filled..head.content_length])
            .map_err(ReceiveError::Read)?;
        if count == 0 {
            return Err(ReceiveError::UnexpectedEof);
        }
        body_filled += count;
    }

    Ok(ReceivedRequest {
        head,
        body: &body_storage[..head.content_length],
    })
}

fn find_header_terminator(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}
