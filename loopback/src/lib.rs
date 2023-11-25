#![no_std]
extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;
use netcore::common::NetError;
use netcore::{EthernetAddress, Medium, NetBufOps, NetDriverOps};

pub struct LoopbackDev {
    queue: VecDeque<Vec<u8>>,
}

impl Default for LoopbackDev {
    fn default() -> Self {
        Self::new()
    }
}

impl LoopbackDev {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
}

impl NetDriverOps for LoopbackDev {
    fn medium(&self) -> Medium {
        Medium::Ip
    }

    fn mac_address(&self) -> EthernetAddress {
        EthernetAddress::from_bytes(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
    }

    fn can_transmit(&self) -> bool {
        true
    }

    fn can_receive(&self) -> bool {
        !self.queue.is_empty()
    }

    fn rx_queue_size(&self) -> usize {
        usize::MAX
    }

    fn tx_queue_size(&self) -> usize {
        usize::MAX
    }

    fn recycle_rx_buffer(&mut self, _rx_buf: Box<dyn NetBufOps>) -> Result<(), NetError> {
        Ok(())
    }

    fn recycle_tx_buffers(&mut self) -> Result<(), NetError> {
        Ok(())
    }

    fn transmit(&mut self, tx_buf: Box<dyn NetBufOps>) -> Result<(), NetError> {
        self.queue.push_back(tx_buf.packet().to_vec());
        Ok(())
    }

    fn receive(&mut self) -> Result<Box<dyn NetBufOps>, NetError> {
        let buf = self.queue.pop_front().unwrap();
        Ok(Box::new(NetBuf(buf)))
    }

    fn alloc_tx_buffer(&mut self, size: usize) -> Result<Box<dyn NetBufOps>, NetError> {
        let mut buffer = vec![0; size];
        buffer.resize(size, 0);
        Ok(Box::new(NetBuf(buffer)))
    }
}

struct NetBuf(Vec<u8>);

impl NetBufOps for NetBuf {
    fn packet(&self) -> &[u8] {
        self.0.as_slice()
    }

    fn packet_mut(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }

    fn packet_len(&self) -> usize {
        self.0.len()
    }
}
