#[cfg(feature = "ble")]
pub mod ble;
pub mod protocol;
#[cfg(feature = "tcp")]
pub mod tcp;

#[cfg(feature = "ble")]
pub use ble::{BleCentralTransport, BlePeripheralTransport};
pub use protocol::ProtocolWrapper;
#[cfg(feature = "tcp")]
pub use tcp::TcpTransport;

#[allow(async_fn_in_trait)]
pub trait RawTransport {
    type Error;

    /// Send raw byte data
    async fn send_bytes(&mut self, data: &[u8]) -> Result<(), Self::Error>;

    /// Receive raw byte data
    async fn receive_bytes(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, Self::Error>;
}
