use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use mushroom_web_licheerv_nano::polling_stream::{
    HTTP_IO_INACTIVITY_TIMEOUT_NANOS, PollingStream, PollingStreamError, ReceiveState, SendState,
    TcpPump,
};
use mushroom_web_licheerv_nano::server::DuplexStream;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestError {
    Receive,
}

enum ReceiveAction {
    Pending,
    Data(Vec<u8>),
    Closed,
    Error(TestError),
}

enum SendAction {
    Pending,
    Capacity(usize),
    Error(TestError),
}

#[derive(Default)]
struct Observation {
    sent: Vec<u8>,
    polls: usize,
}

struct ScriptedPump {
    receive: VecDeque<ReceiveAction>,
    send: VecDeque<SendAction>,
    now_nanos: u64,
    nanos_per_poll: u64,
    observation: Rc<RefCell<Observation>>,
}

impl ScriptedPump {
    fn new(
        receive: impl IntoIterator<Item = ReceiveAction>,
        send: impl IntoIterator<Item = SendAction>,
        nanos_per_poll: u64,
        observation: Rc<RefCell<Observation>>,
    ) -> Self {
        Self {
            receive: receive.into_iter().collect(),
            send: send.into_iter().collect(),
            now_nanos: 0,
            nanos_per_poll,
            observation,
        }
    }
}

impl TcpPump for ScriptedPump {
    type Error = TestError;

    fn poll(&mut self) -> Result<(), Self::Error> {
        self.now_nanos = self.now_nanos.saturating_add(self.nanos_per_poll);
        self.observation.borrow_mut().polls += 1;
        Ok(())
    }

    fn now_nanos(&self) -> u64 {
        self.now_nanos
    }

    fn try_receive(&mut self, destination: &mut [u8]) -> Result<ReceiveState, Self::Error> {
        match self.receive.pop_front().unwrap_or(ReceiveAction::Pending) {
            ReceiveAction::Pending => Ok(ReceiveState::Pending),
            ReceiveAction::Closed => Ok(ReceiveState::Closed),
            ReceiveAction::Error(error) => Err(error),
            ReceiveAction::Data(bytes) => {
                assert!(bytes.len() <= destination.len());
                destination[..bytes.len()].copy_from_slice(&bytes);
                Ok(ReceiveState::Received(bytes.len()))
            }
        }
    }

    fn try_send(&mut self, bytes: &[u8]) -> Result<SendState, Self::Error> {
        match self.send.pop_front().unwrap_or(SendAction::Pending) {
            SendAction::Pending => Ok(SendState::Pending),
            SendAction::Error(error) => Err(error),
            SendAction::Capacity(capacity) => {
                let sent = capacity.min(bytes.len());
                self.observation
                    .borrow_mut()
                    .sent
                    .extend_from_slice(&bytes[..sent]);
                Ok(SendState::Sent(sent))
            }
        }
    }
}

#[test]
fn pending_receive_is_polled_until_exact_data_arrives() {
    let observation = Rc::new(RefCell::new(Observation::default()));
    let pump = ScriptedPump::new(
        [
            ReceiveAction::Pending,
            ReceiveAction::Data(b"hello".to_vec()),
        ],
        [],
        1,
        observation.clone(),
    );
    let mut stream = PollingStream::new(pump);
    let mut destination = [0_u8; 8];

    let count = DuplexStream::read(&mut stream, &mut destination).unwrap();

    assert_eq!(count, 5);
    assert_eq!(&destination[..count], b"hello");
    assert_eq!(observation.borrow().polls, 2);
}

#[test]
fn closed_receive_is_reported_as_eof() {
    let observation = Rc::new(RefCell::new(Observation::default()));
    let pump = ScriptedPump::new([ReceiveAction::Closed], [], 1, observation);
    let mut stream = PollingStream::new(pump);

    assert_eq!(DuplexStream::read(&mut stream, &mut [0_u8; 1]), Ok(0));
}

#[test]
fn receive_pending_until_the_fixed_limit_returns_timeout() {
    let observation = Rc::new(RefCell::new(Observation::default()));
    let pump = ScriptedPump::new(
        [],
        [],
        HTTP_IO_INACTIVITY_TIMEOUT_NANOS / 3,
        observation.clone(),
    );
    let mut stream = PollingStream::new(pump);

    assert_eq!(
        DuplexStream::read(&mut stream, &mut [0_u8; 1]),
        Err(PollingStreamError::Timeout)
    );
    assert_eq!(observation.borrow().polls, 3);
}

#[test]
fn partial_sends_preserve_every_byte_in_order() {
    let observation = Rc::new(RefCell::new(Observation::default()));
    let pump = ScriptedPump::new(
        [],
        [
            SendAction::Capacity(2),
            SendAction::Pending,
            SendAction::Capacity(3),
            SendAction::Capacity(8),
        ],
        1,
        observation.clone(),
    );
    let mut stream = PollingStream::new(pump);

    DuplexStream::write_all(&mut stream, b"abcdefgh").unwrap();

    assert_eq!(observation.borrow().sent, b"abcdefgh");
    assert_eq!(observation.borrow().polls, 4);
}

#[test]
fn send_pending_until_the_fixed_limit_returns_timeout() {
    let observation = Rc::new(RefCell::new(Observation::default()));
    let pump = ScriptedPump::new(
        [],
        [
            SendAction::Pending,
            SendAction::Pending,
            SendAction::Pending,
            SendAction::Error(TestError::Receive),
        ],
        HTTP_IO_INACTIVITY_TIMEOUT_NANOS / 3,
        observation.clone(),
    );
    let mut stream = PollingStream::new(pump);

    assert_eq!(
        DuplexStream::write_all(&mut stream, b"x"),
        Err(PollingStreamError::Timeout)
    );
    assert_eq!(observation.borrow().polls, 3);
}

#[test]
fn receive_transport_failure_preserves_the_underlying_error() {
    let observation = Rc::new(RefCell::new(Observation::default()));
    let pump = ScriptedPump::new(
        [ReceiveAction::Error(TestError::Receive)],
        [],
        1,
        observation,
    );
    let mut stream = PollingStream::new(pump);

    assert_eq!(
        DuplexStream::read(&mut stream, &mut [0_u8; 1]),
        Err(PollingStreamError::Transport(TestError::Receive))
    );
}
