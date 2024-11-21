pub mod device;
pub mod edge;

pub use device::DeviceCommunicator;
pub use edge::EdgeCommunicator;

pub type BleDeviceCommunicator<M> = DeviceCommunicator<crate::transport::BlePeripheralTransport, M>;
pub type TcpEdgeCommunicator<B> = EdgeCommunicator<crate::transport::TcpTransport, B>;

/// Get device MAC address with collision-resistant implementation
pub fn get_device_mac(device_id: i32) -> [u8; 6] {
    let hash = device_id
        .unsigned_abs()
        .wrapping_mul(0x9E3779B9)
        .wrapping_add(0x85EBCA6B);

    [
        0x12,
        0x34,
        0x56,
        (hash >> 16) as u8,
        (hash >> 8) as u8,
        hash as u8,
    ]
}
