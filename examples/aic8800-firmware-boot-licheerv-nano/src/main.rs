#![no_std]
#![no_main]

extern crate axplat_riscv64_licheerv_nano;

use arceos_aic8800_firmware_boot_licheerv_nano::{AicNetworkDevice, DhcpBoundHandler, run};
use smoltcp::iface::{Interface, SocketHandle, SocketSet};
use smoltcp::wire::Ipv4Cidr;

struct StopAfterDhcp;

impl DhcpBoundHandler for StopAfterDhcp {
    fn handle<D: AicNetworkDevice>(
        &mut self,
        _interface: &mut Interface,
        _device: &mut D,
        _sockets: &mut SocketSet<'_>,
        _dhcp_handle: SocketHandle,
        _address: Ipv4Cidr,
    ) {
    }
}

#[unsafe(no_mangle)]
fn main() {
    run(&mut StopAfterDhcp);
}
