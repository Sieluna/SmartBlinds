#[cfg(feature = "ble")]
pub mod ble;
#[cfg(feature = "tcp")]
pub mod tcp;
#[cfg(feature = "udp")]
pub mod udp;

#[cfg(feature = "ble")]
pub use ble::{BleCentralTransport, BlePeripheralTransport};
#[cfg(feature = "tcp")]
pub use tcp::TcpTransport;
#[cfg(feature = "udp")]
pub use udp::{UdpClientTransport, UdpServerTransport, UdpTransport};

pub use lumisync_api::transport::{AsyncMessageTransport, Protocol, SyncMessageTransport};

use embedded_io_async::{ErrorType, Read, Write};

#[allow(async_fn_in_trait)]
pub trait RawTransport {
    type Error;

    /// Send raw byte data
    async fn send_bytes(&mut self, data: &[u8]) -> Result<(), Self::Error>;

    /// Receive raw byte data
    async fn receive_bytes(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, Self::Error>;
}

impl ErrorType for TcpTransport {
    type Error = crate::Error;
}

impl Read for TcpTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self.receive_bytes(buf).await? {
            Some(n) => Ok(n),
            None => Ok(0),
        }
    }
}

impl Write for TcpTransport {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.send_bytes(buf).await?;
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(feature = "ble")]
impl ErrorType for BlePeripheralTransport {
    type Error = crate::Error;
}

#[cfg(feature = "ble")]
impl Read for BlePeripheralTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self.receive_bytes(buf).await? {
            Some(n) => Ok(n),
            None => Ok(0),
        }
    }
}

#[cfg(feature = "ble")]
impl Write for BlePeripheralTransport {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.send_bytes(buf).await?;
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(feature = "udp")]
impl ErrorType for UdpTransport {
    type Error = crate::Error;
}

#[cfg(feature = "udp")]
impl Read for UdpTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self.receive_bytes(buf).await? {
            Some(n) => Ok(n),
            None => Ok(0),
        }
    }
}

#[cfg(feature = "udp")]
impl Write for UdpTransport {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.send_bytes(buf).await?;
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
