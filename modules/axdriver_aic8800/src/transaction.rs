use crate::device::Aic8800Product;
use crate::protocol::{
    ConfigResponse, ProtocolError, build_command_frame, finalize_command_transfer,
};
use crate::response::{AicResponseError, AicResponseIo, read_config_response};
use crate::sdio::{AicCommandError, AicCommandIo, send_command_transfer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicTransactionError<E> {
    Protocol(ProtocolError),
    Send(AicCommandError<E>),
    Receive(AicResponseError<E>),
}

pub fn execute_config_command<'a, I>(
    io: &mut I,
    product: Aic8800Product,
    request_id: u16,
    expected_response_id: u16,
    destination_id: u16,
    source_id: u16,
    parameter: &[u8],
    transmit_storage: &mut [u8],
    receive_storage: &'a mut [u8],
    response_timeout_ms: u32,
) -> Result<ConfigResponse<'a>, AicTransactionError<<I as AicCommandIo>::Error>>
where
    I: AicCommandIo + AicResponseIo<Error = <I as AicCommandIo>::Error>,
{
    let frame_length = build_command_frame(
        transmit_storage,
        product.command_header_mode(),
        request_id,
        destination_id,
        source_id,
        parameter,
    )
    .map_err(AicTransactionError::Protocol)?;
    let transfer_length = finalize_command_transfer(transmit_storage, frame_length)
        .map_err(AicTransactionError::Protocol)?;
    send_command_transfer(io, product, &mut transmit_storage[..transfer_length])
        .map_err(AicTransactionError::Send)?;
    read_config_response(
        io,
        product,
        expected_response_id,
        receive_storage,
        response_timeout_ms,
    )
    .map_err(AicTransactionError::Receive)
}
