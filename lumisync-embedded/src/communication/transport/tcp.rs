use core::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use embassy_net::{IpAddress, IpEndpoint, Stack, tcp::TcpSocket};
use embedded_io_async::{Read, Write};

use crate::{Error, Result};

use super::RawTransport;

pub struct TcpTransport {
    socket: Option<TcpSocket<'static>>,
    server_endpoint: IpEndpoint,
}

impl TcpTransport {
    pub async fn new(
        stack: Stack<'static>,
        server_addr: &str,
        port: u16,
        rx_buffer: &'static mut [u8],
        tx_buffer: &'static mut [u8],
    ) -> Result<Self> {
        let server_ip = server_addr
            .parse::<Ipv4Addr>()
            .map_err(|_| Error::InitializationError)?;

        let server_endpoint = IpEndpoint::from(SocketAddr::V4(SocketAddrV4::new(server_ip, port)));

        let mut socket = TcpSocket::new(stack, rx_buffer, tx_buffer);
        socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

        // Try to connect to server
        socket
            .connect(server_endpoint)
            .await
            .map_err(|_| Error::NetworkError)?;

        Ok(Self {
            socket: Some(socket),
            server_endpoint,
        })
    }

    /// Create a TCP transport that listens for incoming connections
    pub async fn new_server(
        stack: Stack<'static>,
        listen_port: u16,
        rx_buffer: &'static mut [u8],
        tx_buffer: &'static mut [u8],
    ) -> Result<Self> {
        let mut socket = TcpSocket::new(stack, rx_buffer, tx_buffer);
        socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

        let listen_endpoint = IpEndpoint {
            addr: IpAddress::Ipv4(Ipv4Addr::LOCALHOST),
            port: listen_port,
        };

        // Accept incoming connection
        socket
            .accept(listen_endpoint)
            .await
            .map_err(|_| Error::NetworkError)?;

        Ok(Self {
            socket: Some(socket),
            server_endpoint: listen_endpoint,
        })
    }

    /// Check if socket is still connected
    pub fn is_connected(&self) -> bool {
        if let Some(socket) = &self.socket {
            socket.state() == embassy_net::tcp::State::Established
        } else {
            false
        }
    }

    /// Reconnect to server if disconnected
    pub async fn reconnect(&mut self) -> Result<()> {
        if let Some(socket) = &mut self.socket {
            if socket.state() != embassy_net::tcp::State::Established {
                // Close the old socket
                socket.close();

                // Try to reconnect
                socket
                    .connect(self.server_endpoint)
                    .await
                    .map_err(|_| Error::NetworkError)?;
            }
        }
        Ok(())
    }
}

impl RawTransport for TcpTransport {
    type Error = Error;

    async fn send_bytes(&mut self, data: &[u8]) -> Result<()> {
        if let Some(socket) = &mut self.socket {
            // Send length prefix (4 bytes, big-endian)
            let len = data.len() as u32;
            let len_bytes = len.to_be_bytes();
            socket
                .write_all(&len_bytes)
                .await
                .map_err(|_| Error::NetworkError)?;

            // Send actual data
            socket
                .write_all(data)
                .await
                .map_err(|_| Error::NetworkError)?;

            socket.flush().await.map_err(|_| Error::NetworkError)?;
        } else {
            return Err(Error::NetworkError);
        }

        Ok(())
    }

    async fn receive_bytes(&mut self, buffer: &mut [u8]) -> Result<Option<usize>> {
        if let Some(socket) = &mut self.socket {
            // Read length prefix (4 bytes, big-endian)
            let mut len_bytes = [0u8; 4];
            match socket.read_exact(&mut len_bytes).await {
                Ok(_) => {
                    let expected_len = u32::from_be_bytes(len_bytes) as usize;

                    // Ensure we don't exceed buffer size
                    let read_len = expected_len.min(buffer.len());

                    // Read actual data
                    match socket.read_exact(&mut buffer[..read_len]).await {
                        Ok(_) => Ok(Some(read_len)),
                        Err(_) => Ok(None),
                    }
                }
                Err(_) => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
}
