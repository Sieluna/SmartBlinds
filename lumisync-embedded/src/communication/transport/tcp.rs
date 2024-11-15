use core::net::{Ipv4Addr, SocketAddrV4};

use embassy_net::tcp::{State, TcpSocket};
use embassy_net::{IpEndpoint, IpListenEndpoint, Stack};
use embedded_io_async::Write;

use crate::{Error, Result};

use super::RawTransport;

pub struct TcpTransport {
    socket: TcpSocket<'static>,
}

impl TcpTransport {
    pub fn new(
        stack: Stack<'static>,
        rx_buffer: &'static mut [u8],
        tx_buffer: &'static mut [u8],
    ) -> Self {
        let mut socket = TcpSocket::new(stack, rx_buffer, tx_buffer);
        socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

        Self { socket }
    }

    /// Connect to a remote server
    pub async fn connect(&mut self, server_addr: &str, port: u16) -> Result<()> {
        let server_ip = server_addr
            .parse::<Ipv4Addr>()
            .map_err(|_| Error::InitializationError)?;

        let server_endpoint = IpEndpoint::from(SocketAddrV4::new(server_ip, port));

        self.socket
            .connect(server_endpoint)
            .await
            .map_err(|_| Error::NetworkError)?;

        Ok(())
    }

    /// Accept an incoming connection on specified endpoint
    pub async fn accept(&mut self, endpoint: IpListenEndpoint) -> Result<()> {
        self.socket
            .accept(endpoint)
            .await
            .map_err(|_| Error::NetworkError)?;

        Ok(())
    }

    /// Check if socket is connected
    pub fn is_connected(&self) -> bool {
        self.socket.state() == State::Established
    }

    /// Close current connection (socket can be reused)
    pub fn close(&mut self) {
        self.socket.close();
    }

    /// Abort current connection (socket can be reused)
    pub fn abort(&mut self) {
        self.socket.abort();
    }

    /// Get socket state for debugging
    pub fn get_state(&self) -> State {
        self.socket.state()
    }
}

impl RawTransport for TcpTransport {
    type Error = Error;

    async fn send_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.socket
            .write_all(data)
            .await
            .map_err(|_| Error::NetworkError)?;

        self.socket.flush().await.map_err(|_| Error::NetworkError)?;
        Ok(())
    }

    async fn receive_bytes(&mut self, buffer: &mut [u8]) -> Result<Option<usize>> {
        match self.socket.read(buffer).await {
            Ok(0) => Ok(None), // EOF
            Ok(n) => Ok(Some(n)),
            Err(_) => Ok(None),
        }
    }
}
