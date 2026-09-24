extern crate alloc;

use alloc::boxed::Box;

use arceos_aic8800_firmware_boot_licheerv_nano::{
    AicNetworkDevice, DhcpBoundHandler, run as run_aic8800,
};
use axmodel_mushroom_yolov5::engine::TpuEngine;
use axstd::{println, vec};
use smoltcp::iface::{Interface, SocketHandle, SocketSet};
use smoltcp::socket::{dhcpv4, tcp};
use smoltcp::time::{Duration, Instant};
use smoltcp::wire::{IpCidr, Ipv4Address, Ipv4Cidr};

use crate::backend::InferenceBackendState;
use crate::envelope::TOTAL_LENGTH;
use crate::polling_stream::{PollingStream, ReceiveState, SendState, TcpPump};
use crate::server::{ServeOutcome, serve_one};
use crate::service::InferenceLock;
use crate::stream::MAX_REQUEST_HEADER_LENGTH;

const HTTP_PORT: u16 = 80;
const TCP_RX_BUFFER_LENGTH: usize = 64 * 1024;
const TCP_TX_BUFFER_LENGTH: usize = 64 * 1024;
const TCP_CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HardwareNetworkError {
    TransportFailed,
    DhcpNotBound,
    Receive,
    Send,
}

enum DhcpUpdate {
    None,
    Deconfigured,
    Configured {
        address: Ipv4Cidr,
        router: Option<Ipv4Address>,
    },
}

fn current_time() -> Instant {
    Instant::from_micros_const((axhal::time::wall_time_nanos() / 1_000) as i64)
}

fn poll_network<D: AicNetworkDevice>(
    interface: &mut Interface,
    device: &mut D,
    sockets: &mut SocketSet<'_>,
    dhcp_handle: SocketHandle,
    dhcp_bound: &mut bool,
) -> Result<(), HardwareNetworkError> {
    interface.poll(current_time(), device, sockets);
    if device.transport_failed() {
        return Err(HardwareNetworkError::TransportFailed);
    }

    let update = match sockets.get_mut::<dhcpv4::Socket>(dhcp_handle).poll() {
        Some(dhcpv4::Event::Configured(config)) => DhcpUpdate::Configured {
            address: config.address,
            router: config.router,
        },
        Some(dhcpv4::Event::Deconfigured) => DhcpUpdate::Deconfigured,
        None => DhcpUpdate::None,
    };
    match update {
        DhcpUpdate::None => {}
        DhcpUpdate::Deconfigured => {
            interface.update_ip_addrs(|addresses| addresses.clear());
            interface.routes_mut().remove_default_ipv4_route();
            *dhcp_bound = false;
            println!("MUSHROOM_WEB_DHCP_DECONFIGURED");
        }
        DhcpUpdate::Configured { address, router } => {
            interface.update_ip_addrs(|addresses| {
                addresses.clear();
                addresses.push(IpCidr::Ipv4(address)).unwrap();
            });
            interface.routes_mut().remove_default_ipv4_route();
            if let Some(router) = router {
                interface
                    .routes_mut()
                    .add_default_ipv4_route(router)
                    .unwrap();
            }
            *dhcp_bound = true;
            println!("MUSHROOM_WEB_DHCP_BOUND address={address}");
            println!("MUSHROOM_WEB_URL http://{}/", address.address());
        }
    }
    Ok(())
}

struct SmoltcpPump<'a, 's, D> {
    interface: &'a mut Interface,
    device: &'a mut D,
    sockets: &'a mut SocketSet<'s>,
    dhcp_handle: SocketHandle,
    tcp_handle: SocketHandle,
    dhcp_bound: &'a mut bool,
}

impl<D: AicNetworkDevice> TcpPump for SmoltcpPump<'_, '_, D> {
    type Error = HardwareNetworkError;

    fn poll(&mut self) -> Result<(), Self::Error> {
        poll_network(
            self.interface,
            self.device,
            self.sockets,
            self.dhcp_handle,
            self.dhcp_bound,
        )?;
        if !*self.dhcp_bound {
            return Err(HardwareNetworkError::DhcpNotBound);
        }
        Ok(())
    }

    fn now_nanos(&self) -> u64 {
        axhal::time::wall_time_nanos()
    }

    fn try_receive(&mut self, destination: &mut [u8]) -> Result<ReceiveState, Self::Error> {
        let socket = self.sockets.get_mut::<tcp::Socket>(self.tcp_handle);
        if socket.can_recv() {
            return socket
                .recv_slice(destination)
                .map(ReceiveState::Received)
                .map_err(|_| HardwareNetworkError::Receive);
        }
        if !socket.may_recv() {
            return Ok(ReceiveState::Closed);
        }
        Ok(ReceiveState::Pending)
    }

    fn try_send(&mut self, bytes: &[u8]) -> Result<SendState, Self::Error> {
        let socket = self.sockets.get_mut::<tcp::Socket>(self.tcp_handle);
        if socket.can_send() {
            return socket
                .send_slice(bytes)
                .map(SendState::Sent)
                .map_err(|_| HardwareNetworkError::Send);
        }
        if !socket.may_send() {
            return Ok(SendState::Closed);
        }
        Ok(SendState::Pending)
    }
}

struct ProductHandler;

impl DhcpBoundHandler for ProductHandler {
    fn handle<D: AicNetworkDevice>(
        &mut self,
        interface: &mut Interface,
        device: &mut D,
        sockets: &mut SocketSet<'_>,
        dhcp_handle: SocketHandle,
        address: Ipv4Cidr,
    ) {
        run_http_server(interface, device, sockets, dhcp_handle, address)
    }
}

pub fn run() {
    run_aic8800(&mut ProductHandler);
}

pub fn run_http_server<D: AicNetworkDevice>(
    interface: &mut Interface,
    device: &mut D,
    sockets: &mut SocketSet<'_>,
    dhcp_handle: SocketHandle,
    address: Ipv4Cidr,
) -> ! {
    let mut backend = match TpuEngine::initialize() {
        Ok(engine) => {
            let timing = engine.initialization_timing();
            println!(
                "MUSHROOM_TPU_READY load_us={} prepare_us={} hardware_us={}",
                timing.load_model_us, timing.prepare_dmabuf_us, timing.cache_and_hardware_us
            );
            InferenceBackendState::ready(engine)
        }
        Err(error) => {
            println!("MUSHROOM_TPU_UNAVAILABLE error={error:?}");
            InferenceBackendState::unavailable()
        }
    };
    let inference_lock = InferenceLock::new();
    let mut header_storage = Box::new([0_u8; MAX_REQUEST_HEADER_LENGTH]);
    let mut body_storage = vec![0_u8; TOTAL_LENGTH];

    let rx_buffer = tcp::SocketBuffer::new(vec![0_u8; TCP_RX_BUFFER_LENGTH]);
    let tx_buffer = tcp::SocketBuffer::new(vec![0_u8; TCP_TX_BUFFER_LENGTH]);
    let mut tcp_socket = tcp::Socket::new(rx_buffer, tx_buffer);
    tcp_socket.set_timeout(Some(TCP_CONNECTION_TIMEOUT));
    let tcp_handle = sockets.add(tcp_socket);
    let mut dhcp_bound = true;
    println!("MUSHROOM_WEB_URL http://{}/", address.address());

    loop {
        if let Err(error) = poll_network(interface, device, sockets, dhcp_handle, &mut dhcp_bound) {
            println!("MUSHROOM_WEB_NETWORK_FATAL error={error:?}");
            loop {
                core::hint::spin_loop();
            }
        }

        let request_ready = {
            let socket = sockets.get_mut::<tcp::Socket>(tcp_handle);
            if !dhcp_bound {
                socket.abort();
                false
            } else {
                if !socket.is_open() {
                    socket.abort();
                    if let Err(error) = socket.listen(HTTP_PORT) {
                        println!("MUSHROOM_WEB_LISTEN_FAILED error={error:?}");
                    } else {
                        println!("MUSHROOM_WEB_LISTENING port={HTTP_PORT}");
                    }
                }
                socket.can_recv()
            }
        };
        if !request_ready {
            core::hint::spin_loop();
            continue;
        }

        let readiness = backend.readiness(true);
        let result = {
            let pump = SmoltcpPump {
                interface,
                device,
                sockets,
                dhcp_handle,
                tcp_handle,
                dhcp_bound: &mut dhcp_bound,
            };
            let mut stream = PollingStream::new(pump);
            serve_one(
                &mut stream,
                &mut header_storage,
                &mut body_storage,
                readiness,
                &inference_lock,
                &mut backend,
            )
        };

        let socket = sockets.get_mut::<tcp::Socket>(tcp_handle);
        match result {
            Ok(ServeOutcome::Responded) => {
                println!("MUSHROOM_WEB_REQUEST_RESPONDED");
                socket.close();
            }
            Ok(ServeOutcome::InferenceCompleted(report)) => {
                println!(
                    "MUSHROOM_INFERENCE_PASS request_id={} input_crc32={:08x} detections={} receive_us={} quantize_us={} tpu_us={} output_sync_us={} postprocess_us={} total_us={}",
                    report.request_id,
                    report.input_crc32,
                    report.detection_count,
                    report.receive_us,
                    report.timing.quantize_us,
                    report.timing.tpu_us,
                    report.timing.output_sync_us,
                    report.timing.postprocess_us,
                    report.timing.total_us,
                );
                socket.close();
            }
            Ok(ServeOutcome::PeerClosed) => {
                println!("MUSHROOM_WEB_REQUEST_PEER_CLOSED");
                socket.abort();
            }
            Err(error) => {
                println!("MUSHROOM_WEB_REQUEST_FAILED error={error:?}");
                socket.abort();
            }
        }
    }
}
