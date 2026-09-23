use arceos_mushroom_web_licheerv_nano::envelope::TOTAL_LENGTH;
use arceos_mushroom_web_licheerv_nano::http::RequestRoute;
use arceos_mushroom_web_licheerv_nano::stream::{
    ByteReader, MAX_REQUEST_HEADER_LENGTH, ReceiveError, receive_request,
};

struct ChunkReader {
    bytes: Vec<u8>,
    position: usize,
    chunk_size: usize,
}

impl ChunkReader {
    fn new(bytes: Vec<u8>, chunk_size: usize) -> Self {
        Self {
            bytes,
            position: 0,
            chunk_size,
        }
    }
}

impl ByteReader for ChunkReader {
    type Error = ();

    fn read(&mut self, destination: &mut [u8]) -> Result<usize, Self::Error> {
        if self.position == self.bytes.len() {
            return Ok(0);
        }
        let count = destination
            .len()
            .min(self.chunk_size)
            .min(self.bytes.len() - self.position);
        destination[..count].copy_from_slice(&self.bytes[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}

fn infer_head() -> Vec<u8> {
    format!(
        "POST /api/infer HTTP/1.1\r\nContent-Type: application/vnd.arceos.rgb-u8\r\nContent-Length: {TOTAL_LENGTH}\r\n\r\n"
    )
    .into_bytes()
}

#[test]
fn receives_a_header_and_body_split_across_small_reads() {
    let mut request = infer_head();
    request.extend((0..TOTAL_LENGTH).map(|index| (index & 0xff) as u8));
    let mut reader = ChunkReader::new(request, 37);
    let mut header = [0_u8; MAX_REQUEST_HEADER_LENGTH];
    let mut body = vec![0_u8; TOTAL_LENGTH];

    let received = receive_request(&mut reader, &mut header, &mut body).unwrap();

    assert_eq!(received.head.route, RequestRoute::Infer);
    assert_eq!(received.body.len(), TOTAL_LENGTH);
    assert_eq!(received.body[0], 0);
    assert_eq!(received.body[257], 1);
    assert_eq!(
        received.body[TOTAL_LENGTH - 1],
        ((TOTAL_LENGTH - 1) & 0xff) as u8
    );
}

#[test]
fn preserves_body_bytes_received_in_the_same_read_as_the_header() {
    let mut request = infer_head();
    request.extend(vec![0x5a; TOTAL_LENGTH]);
    let first_read_size = request.len();
    let mut reader = ChunkReader::new(request, first_read_size);
    let mut header = [0_u8; MAX_REQUEST_HEADER_LENGTH];
    let mut body = vec![0_u8; TOTAL_LENGTH];

    let received = receive_request(&mut reader, &mut header, &mut body).unwrap();

    assert!(received.body.iter().all(|byte| *byte == 0x5a));
}

#[test]
fn reports_truncated_body_without_returning_partial_input() {
    let mut request = infer_head();
    request.extend(vec![0x11; TOTAL_LENGTH - 1]);
    let mut reader = ChunkReader::new(request, 1024);
    let mut header = [0_u8; MAX_REQUEST_HEADER_LENGTH];
    let mut body = vec![0_u8; TOTAL_LENGTH];

    assert_eq!(
        receive_request(&mut reader, &mut header, &mut body),
        Err(ReceiveError::UnexpectedEof)
    );
}

#[test]
fn rejects_a_header_that_exceeds_the_fixed_storage() {
    let request = vec![b'a'; MAX_REQUEST_HEADER_LENGTH + 1];
    let mut reader = ChunkReader::new(request, 512);
    let mut header = [0_u8; MAX_REQUEST_HEADER_LENGTH];
    let mut body = vec![0_u8; TOTAL_LENGTH];

    assert_eq!(
        receive_request(&mut reader, &mut header, &mut body),
        Err(ReceiveError::HeaderTooLarge)
    );
}

#[test]
fn receives_get_without_allocating_a_body_from_the_stream() {
    let mut reader = ChunkReader::new(b"GET /health HTTP/1.1\r\nHost: board\r\n\r\n".to_vec(), 5);
    let mut header = [0_u8; MAX_REQUEST_HEADER_LENGTH];
    let mut body = vec![0_u8; TOTAL_LENGTH];

    let received = receive_request(&mut reader, &mut header, &mut body).unwrap();

    assert_eq!(received.head.route, RequestRoute::Health);
    assert!(received.body.is_empty());
}
