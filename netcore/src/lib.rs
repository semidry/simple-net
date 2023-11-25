#![feature(new_uninit)]
#![feature(ip_in_core)]
#![no_std]

extern crate alloc;

use crate::common::NetError;
use crate::interface::{NetInterface, NetInterfaceWrapper, SocketSetWrapper};
use crate::listen_table::ListenTable;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::any::Any;
use preprint::pprintln;
use smoltcp::time::Instant;
use smoltcp::wire::IpAddress;
use spin::{Lazy, Once};

mod addr;
pub mod common;
mod interface;
mod listen_table;

mod device;
pub mod tcp;
pub mod udp;
use crate::device::NetDeviceWrapper;
pub use smoltcp::phy::Medium;
pub use smoltcp::wire::EthernetAddress;

pub static NET_INTERFACE: Once<NetInterfaceWrapper> = Once::new();
pub static SOCKET_SET: Lazy<SocketSetWrapper> = Lazy::new(SocketSetWrapper::new);
pub static LISTENING_TABLE: Lazy<ListenTable> = Lazy::new(ListenTable::new);
pub static KERNEL_NET_FUNC: Once<Arc<dyn KernelNetFunc>> = Once::new();

pub struct NetInstant {
    pub micros: i64,
}

impl From<NetInstant> for Instant {
    fn from(val: NetInstant) -> Self {
        Instant::from_micros(val.micros)
    }
}

pub trait KernelNetFunc: Send + Sync {
    fn now(&self) -> NetInstant;
    fn yield_now(&self) -> bool; // equal to suspend in kernel
}

pub trait NetBufOps: Any {
    fn packet(&self) -> &[u8];
    fn packet_mut(&mut self) -> &mut [u8];
    fn packet_len(&self) -> usize;
}

/// Operations that require a network device (NIC) driver to implement.
pub trait NetDriverOps: Send + Sync {
    fn medium(&self) -> Medium;
    /// The ethernet address of the NIC.
    fn mac_address(&self) -> EthernetAddress;

    /// Whether can transmit packets.
    fn can_transmit(&self) -> bool;

    /// Whether can receive packets.
    fn can_receive(&self) -> bool;

    /// Size of the receive queue.
    fn rx_queue_size(&self) -> usize;

    /// Size of the transmit queue.
    fn tx_queue_size(&self) -> usize;

    /// Gives back the `rx_buf` to the receive queue for later receiving.
    ///
    /// `rx_buf` should be the same as the one returned by
    /// [`NetDriverOps::receive`].
    fn recycle_rx_buffer(&mut self, rx_buf: Box<dyn NetBufOps>) -> Result<(), NetError>;

    /// Poll the transmit queue and gives back the buffers for previous transmiting.
    /// returns [`DevResult`].
    fn recycle_tx_buffers(&mut self) -> Result<(), NetError>;

    /// Transmits a packet in the buffer to the network, without blocking,
    /// returns [`DevResult`].
    fn transmit(&mut self, tx_buf: Box<dyn NetBufOps>) -> Result<(), NetError>;

    /// Receives a packet from the network and store it in the [`NetBuf`],
    /// returns the buffer.
    ///
    /// Before receiving, the driver should have already populated some buffers
    /// in the receive queue by [`NetDriverOps::recycle_rx_buffer`].
    ///
    /// If currently no incomming packets, returns an error with type
    /// [`DevError::Again`].
    fn receive(&mut self) -> Result<Box<dyn NetBufOps>, NetError>;

    /// Allocate a memory buffer of a specified size for network transmission,
    /// returns [`DevResult`]
    fn alloc_tx_buffer(&mut self, size: usize) -> Result<Box<dyn NetBufOps>, NetError>;
}

pub fn init_net(
    device: Box<dyn NetDriverOps>,
    kernel_func: Arc<dyn KernelNetFunc>,
    ip: IpAddress,
    gate_way: IpAddress,
    test: bool,
) {
    let mac_addr = EthernetAddress::from_bytes(device.mac_address().as_bytes());
    let mut device = NetDeviceWrapper::new(device, kernel_func.clone());
    if test {
        device.bench_transmit_bandwidth();
    }
    let iface = NetInterfaceWrapper::new(device, kernel_func.clone(), mac_addr);
    iface.setup_ip_addr(ip, 24);
    iface.setup_gateway(gate_way);
    KERNEL_NET_FUNC.call_once(|| kernel_func);
    NET_INTERFACE.call_once(|| iface);
    pprintln!("created net interface");
    pprintln!("  ether:    {}", mac_addr);
    pprintln!("  ip:       {}/{}", ip, 24);
    pprintln!("  gateway:  {}", gate_way);
}

/// Poll the network stack.
///
/// It may receive packets from the NIC and process them, and transmit queued
/// packets to the NIC.
pub fn poll_interfaces() {
    SOCKET_SET.poll_interfaces();
}
