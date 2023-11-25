#![no_std]
extern crate alloc;
use alloc::boxed::Box;
use core::any::Any;
use netcore::common::NetError;
use netcore::{EthernetAddress, Medium, NetBufOps, NetDriverOps};
use virtio_drivers::device::net::{RxBuffer, TxBuffer, VirtIONet};
use virtio_drivers::transport::Transport;
use virtio_drivers::Hal;

pub struct VirtIONetDeviceWrapper<H: Hal, T: Transport, const QS: usize> {
    inner: VirtIONet<H, T, QS>,
}

impl<H: Hal, T: Transport, const QS: usize> VirtIONetDeviceWrapper<H, T, QS> {
    pub fn new(transport: T, buf_len: usize) -> Self {
        let device = VirtIONet::<H, T, QS>::new(transport, buf_len).unwrap();
        VirtIONetDeviceWrapper { inner: device }
    }
}
unsafe impl<H: Hal, T: Transport, const QS: usize> Sync for VirtIONetDeviceWrapper<H, T, QS> {}
unsafe impl<H: Hal, T: Transport, const QS: usize> Send for VirtIONetDeviceWrapper<H, T, QS> {}

impl<H: Hal, T: Transport, const QS: usize> NetDriverOps for VirtIONetDeviceWrapper<H, T, QS> {
    fn medium(&self) -> Medium {
        Medium::Ethernet
    }

    fn mac_address(&self) -> EthernetAddress {
        EthernetAddress(self.inner.mac_address())
    }

    fn can_transmit(&self) -> bool {
        self.inner.can_send()
    }

    fn can_receive(&self) -> bool {
        self.inner.can_recv()
    }

    fn rx_queue_size(&self) -> usize {
        QS
    }

    fn tx_queue_size(&self) -> usize {
        QS
    }

    fn recycle_rx_buffer(&mut self, rx_buf: Box<dyn NetBufOps>) -> Result<(), NetError> {
        let rx_buf =
            unsafe { core::mem::transmute::<Box<dyn NetBufOps>, Box<dyn Any + Send>>(rx_buf) };
        let rx_buf = rx_buf.downcast::<RxBufWrapper>().unwrap();
        self.inner.recycle_rx_buffer(rx_buf.0).unwrap();
        Ok(())
    }

    fn recycle_tx_buffers(&mut self) -> Result<(), NetError> {
        Ok(())
    }

    fn transmit(&mut self, tx_buf: Box<dyn NetBufOps>) -> Result<(), NetError> {
        let tx_buf =
            unsafe { core::mem::transmute::<Box<dyn NetBufOps>, Box<dyn Any + Send>>(tx_buf) };
        let tx_buf = tx_buf.downcast::<TxBufWrapper>().unwrap();
        self.inner.send(tx_buf.0).unwrap();
        Ok(())
    }

    fn receive(&mut self) -> Result<Box<dyn NetBufOps>, NetError> {
        let buf = self.inner.receive().unwrap();
        Ok(Box::new(RxBufWrapper(buf)))
    }

    fn alloc_tx_buffer(&mut self, size: usize) -> Result<Box<dyn NetBufOps>, NetError> {
        let buf = self.inner.new_tx_buffer(size);
        Ok(Box::new(TxBufWrapper(buf)))
    }
}

struct RxBufWrapper(RxBuffer);

impl NetBufOps for RxBufWrapper {
    fn packet(&self) -> &[u8] {
        self.0.packet()
    }

    fn packet_mut(&mut self) -> &mut [u8] {
        self.0.packet_mut()
    }

    fn packet_len(&self) -> usize {
        self.0.packet_len()
    }
}
struct TxBufWrapper(TxBuffer);

impl NetBufOps for TxBufWrapper {
    fn packet(&self) -> &[u8] {
        self.0.packet()
    }

    fn packet_mut(&mut self) -> &mut [u8] {
        self.0.packet_mut()
    }

    fn packet_len(&self) -> usize {
        self.0.packet_len()
    }
}

// fn virtio2core(e:virtio_drivers::Error)->NetError{
//     match e {
//         Error::NotReady => NetError::DeviceError,
//         Error::QueueFull => NetError::DeviceError,
//         Error::WrongToken => NetError::DeviceError,
//         Error::AlreadyUsed => NetError::DeviceError,
//         Error::InvalidParam => NetError::InvalidInput,
//         Error::DmaError => NetError::DeviceError,
//         Error::IoError => NetError::DeviceError,
//         Error::Unsupported => NetError::DeviceError,
//         Error::ConfigSpaceTooSmall => NetError::DeviceError,
//         Error::ConfigSpaceMissing => NetError::DeviceError,
//         Error::SocketDeviceError(_) => NetError::DeviceError,
//     }
// }
