# simple-net

This repository mainly refers to [Arceos ](https://github.com/rcore-os/arceos) and [smoltcp](https://github.com/smoltcp-rs/smoltcp) .

## Usage

```rust
// The core
netcore = {git = "https://github.com/os-module/simple-net"}
// For Qemu virtio-net-device
virtio-net = {git = "https://github.com/os-module/simple-net"}
// For A loopback
loopback = {git = "https://github.com/os-module/simple-net"}
```

```rust
pub fn init_net(
    device: Box<dyn NetDriverOps>,
    kernel_func: Arc<dyn KernelNetFunc>,
    ip: IpAddress,
    gate_way: IpAddress,
    test: bool,
);
netcore::init_net(
                device,
                Arc::new(NetNeedFunc),
                IpAddress::from_str(QEMU_IP).unwrap(),
            	IpAddress::from_str(QEMU_GATEWAY).unwrap(),
                true
            );
```



If you want to specify a new NIC, please implement the following traits.

```rust
pub struct NetInstant {
    pub micros: i64,
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
```



## TODO

- [ ] Multiple devices