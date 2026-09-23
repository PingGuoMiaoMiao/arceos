use crate::server::DuplexStream;

pub const HTTP_IO_INACTIVITY_TIMEOUT_NANOS: u64 = 30_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiveState {
    Received(usize),
    Pending,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendState {
    Sent(usize),
    Pending,
    Closed,
}

pub trait TcpPump {
    type Error;

    fn poll(&mut self) -> Result<(), Self::Error>;
    fn now_nanos(&self) -> u64;
    fn try_receive(&mut self, destination: &mut [u8]) -> Result<ReceiveState, Self::Error>;
    fn try_send(&mut self, bytes: &[u8]) -> Result<SendState, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PollingStreamError<E> {
    Transport(E),
    Closed,
    Timeout,
}

pub struct PollingStream<P> {
    pump: P,
}

impl<P> PollingStream<P> {
    pub const fn new(pump: P) -> Self {
        Self { pump }
    }
}

impl<P: TcpPump> DuplexStream for PollingStream<P> {
    type ReadError = PollingStreamError<P::Error>;
    type WriteError = PollingStreamError<P::Error>;

    fn read(&mut self, destination: &mut [u8]) -> Result<usize, Self::ReadError> {
        if destination.is_empty() {
            return Ok(0);
        }
        let progress_at = self.pump.now_nanos();
        loop {
            self.pump.poll().map_err(PollingStreamError::Transport)?;
            match self
                .pump
                .try_receive(destination)
                .map_err(PollingStreamError::Transport)?
            {
                ReceiveState::Received(count) => return Ok(count),
                ReceiveState::Closed => return Ok(0),
                ReceiveState::Pending => {
                    if self.pump.now_nanos().saturating_sub(progress_at)
                        >= HTTP_IO_INACTIVITY_TIMEOUT_NANOS
                    {
                        return Err(PollingStreamError::Timeout);
                    }
                    core::hint::spin_loop();
                }
            }
        }
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::WriteError> {
        let mut sent = 0;
        let mut progress_at = self.pump.now_nanos();
        while sent < bytes.len() {
            self.pump.poll().map_err(PollingStreamError::Transport)?;
            match self
                .pump
                .try_send(&bytes[sent..])
                .map_err(PollingStreamError::Transport)?
            {
                SendState::Sent(count) => {
                    assert!(count <= bytes.len() - sent);
                    if count != 0 {
                        sent += count;
                        progress_at = self.pump.now_nanos();
                    }
                }
                SendState::Closed => return Err(PollingStreamError::Closed),
                SendState::Pending => {}
            }
            if self.pump.now_nanos().saturating_sub(progress_at) >= HTTP_IO_INACTIVITY_TIMEOUT_NANOS
            {
                return Err(PollingStreamError::Timeout);
            }
            core::hint::spin_loop();
        }
        Ok(())
    }
}
