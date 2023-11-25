use crate::common::{NetError, STANDARD_MTU};
use crate::{KernelNetFunc, NetBufOps, NetDriverOps, LISTENING_TABLE};
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::cell::RefCell;
use log::{info, warn};
use preprint::pprintln;
use smoltcp::iface::SocketSet;
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;

pub struct NetDeviceWrapper {
    inner: RefCell<Box<dyn NetDriverOps>>,
    timer: Arc<dyn KernelNetFunc>,
}

impl NetDeviceWrapper {
    pub fn new(dev: Box<dyn NetDriverOps>, timer: Arc<dyn KernelNetFunc>) -> Self {
        Self {
            inner: RefCell::new(dev),
            timer,
        }
    }
}

impl Device for NetDeviceWrapper {
    type RxToken<'a> = NetRxToken<'a> where Self: 'a;
    type TxToken<'a> = NetTxToken<'a> where Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let mut dev = self.inner.borrow_mut();
        if let Err(e) = dev.recycle_tx_buffers() {
            warn!("recycle_tx_buffers failed: {:?}", e);
            return None;
        }
        if !dev.can_receive() {
            return None;
        }
        match dev.receive() {
            Ok(buf) => Some((NetRxToken(&self.inner, buf), NetTxToken(&self.inner))),
            Err(e) => {
                if !matches!(e, NetError::Again) {
                    warn!("receive failed: {:?}", e);
                }
                None
            }
        }
    }
    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        let mut dev = self.inner.borrow_mut();
        if let Err(e) = dev.recycle_tx_buffers() {
            warn!("recycle_tx_buffers failed: {:?}", e);
            return None;
        }
        if !dev.can_transmit() {
            return None;
        }
        Some(NetTxToken(&self.inner))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1514;
        caps.max_burst_size = None;
        caps.medium = self.inner.borrow().medium();
        caps
    }
}

pub struct NetRxToken<'a>(&'a RefCell<Box<dyn NetDriverOps>>, Box<dyn NetBufOps>);
pub struct NetTxToken<'a>(&'a RefCell<Box<dyn NetDriverOps>>);

impl RxToken for NetRxToken<'_> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut rx_buf = self.1;
        info!("RECV {} bytes", rx_buf.packet_len(),);
        let result = f(rx_buf.packet_mut());
        self.0.borrow_mut().recycle_rx_buffer(rx_buf).unwrap();
        result
    }
    fn preprocess(&self, sockets: &mut SocketSet<'_>) {
        let dev = self.0.borrow_mut();
        let medium = dev.medium();
        snoop_tcp_packet(self.1.packet(), sockets, medium == Medium::Ethernet).ok();
    }
}

impl TxToken for NetTxToken<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut dev = self.0.borrow_mut();
        let mut tx_buf = dev.alloc_tx_buffer(len).unwrap();
        let result = f(tx_buf.packet_mut());
        info!("SEND {} bytes", tx_buf.packet_len());
        dev.transmit(tx_buf).unwrap();
        result
    }
}

fn snoop_tcp_packet(
    buf: &[u8],
    sockets: &mut SocketSet<'_>,
    is_ethernet: bool,
) -> Result<(), smoltcp::wire::Error> {
    use smoltcp::wire::{EthernetFrame, IpProtocol, Ipv4Packet, TcpPacket};

    let ipv4_packet = if is_ethernet {
        let ether_frame = EthernetFrame::new_checked(buf)?;
        Ipv4Packet::new_checked(ether_frame.payload())?
    } else {
        Ipv4Packet::new_checked(buf)?
    };
    if ipv4_packet.next_header() == IpProtocol::Tcp {
        let tcp_packet = TcpPacket::new_checked(ipv4_packet.payload())?;
        let src_addr = (ipv4_packet.src_addr(), tcp_packet.src_port()).into();
        let dst_addr = (ipv4_packet.dst_addr(), tcp_packet.dst_port()).into();
        let is_first = tcp_packet.syn() && !tcp_packet.ack();
        if is_first {
            info!("TCP SYN packet: {} -> {}", src_addr, dst_addr);
            // create a socket for the first incoming TCP packet, as the later accept() returns.
            LISTENING_TABLE.incoming_tcp_packet(src_addr, dst_addr, sockets);
        }
    }
    Ok(())
}

const GB: usize = 1000 * MB;
const MB: usize = 1000 * KB;
const KB: usize = 1000;

impl NetDeviceWrapper {
    pub fn bench_transmit_bandwidth(&mut self) {
        // 10 Gb
        const MAX_SEND_BYTES: usize = GB;
        let mut send_bytes: usize = 0;
        let mut past_send_bytes: usize = 0;
        let mut past_time: Instant = self.timer.now().into();

        // Send bytes
        while send_bytes < MAX_SEND_BYTES {
            if let Some(tx_token) = self.transmit(self.timer.now().into()) {
                NetTxToken::consume(tx_token, STANDARD_MTU, |tx_buf| {
                    tx_buf[0..12].fill(1);
                    // ether type: IPv4
                    tx_buf[12..14].copy_from_slice(&[0x08, 0x00]);
                    tx_buf[14..STANDARD_MTU].fill(1);
                });
                send_bytes += STANDARD_MTU;
            }

            let current_time: Instant = self.timer.now().into();
            if (current_time - past_time).secs() == 1 {
                let gb = ((send_bytes - past_send_bytes) * 8) / GB;
                let mb = (((send_bytes - past_send_bytes) * 8) % GB) / MB;
                let gib = (send_bytes - past_send_bytes) / GB;
                let mib = ((send_bytes - past_send_bytes) % GB) / MB;
                pprintln!(
                    "Transmit: {}.{:03}GBytes, Bandwidth: {}.{:03}Gbits/sec.",
                    gib,
                    mib,
                    gb,
                    mb
                );
                past_time = current_time;
                past_send_bytes = send_bytes;
            }
        }
    }
    #[allow(unused)]
    pub fn bench_receive_bandwidth(&mut self) {
        // 10 Gb
        const MAX_RECEIVE_BYTES: usize = 10 * GB;
        let mut receive_bytes: usize = 0;
        let mut past_receive_bytes: usize = 0;
        let mut past_time: Instant = self.timer.now().into();
        // Receive bytes
        while receive_bytes < MAX_RECEIVE_BYTES {
            if let Some(rx_token) = self.receive(self.timer.now().into()) {
                NetRxToken::consume(rx_token.0, |rx_buf| {
                    receive_bytes += rx_buf.len();
                });
            }

            let current_time: Instant = self.timer.now().into();
            if (current_time - past_time).secs() == 1 {
                let gb = ((receive_bytes - past_receive_bytes) * 8) / GB;
                let mb = (((receive_bytes - past_receive_bytes) * 8) % GB) / MB;
                let gib = (receive_bytes - past_receive_bytes) / GB;
                let mib = ((receive_bytes - past_receive_bytes) % GB) / MB;
                pprintln!(
                    "Receive: {}.{:03}GBytes, Bandwidth: {}.{:03}Gbits/sec.",
                    gib,
                    mib,
                    gb,
                    mb
                );
                past_time = current_time;
                past_receive_bytes = receive_bytes;
            }
        }
    }
}
