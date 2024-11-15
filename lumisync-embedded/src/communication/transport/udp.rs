use core::net::{Ipv4Addr, SocketAddrV4};

use embassy_net::udp::PacketMetadata;
use embassy_net::{IpEndpoint, Stack, udp::UdpSocket};

use crate::{Error, Result};

use super::RawTransport;

pub struct UdpTransport {
    socket: UdpSocket<'static>,
    remote_endpoint: Option<IpEndpoint>,
}

impl UdpTransport {
    pub fn new(
        stack: Stack<'static>,
        rx_meta: &'static mut [PacketMetadata],
        rx_buffer: &'static mut [u8],
        tx_meta: &'static mut [PacketMetadata],
        tx_buffer: &'static mut [u8],
    ) -> Self {
        let socket = UdpSocket::new(stack, rx_meta, rx_buffer, tx_meta, tx_buffer);

        Self {
            socket,
            remote_endpoint: None,
        }
    }

    pub fn bind(&mut self, local_port: u16) -> Result<()> {
        self.socket
            .bind(local_port)
            .map_err(|_| Error::NetworkError)?;
        Ok(())
    }

    /// Set remote endpoint for client mode
    pub fn set_remote(&mut self, remote_addr: &str, remote_port: u16) -> Result<()> {
        let remote_ip = remote_addr
            .parse::<Ipv4Addr>()
            .map_err(|_| Error::InitializationError)?;

        self.remote_endpoint = Some(IpEndpoint::from(SocketAddrV4::new(remote_ip, remote_port)));
        Ok(())
    }

    /// Set remote endpoint directly
    pub fn set_remote_endpoint(&mut self, endpoint: IpEndpoint) {
        self.remote_endpoint = Some(endpoint);
    }

    /// Get current remote endpoint
    pub fn get_remote_endpoint(&self) -> Option<IpEndpoint> {
        self.remote_endpoint
    }

    /// Send data to specific endpoint
    pub async fn send_to(&mut self, data: &[u8], endpoint: IpEndpoint) -> Result<()> {
        self.socket
            .send_to(data, endpoint)
            .await
            .map_err(|_| Error::NetworkError)?;
        Ok(())
    }

    /// Receive data and return sender's endpoint
    pub async fn receive_from(&mut self, buffer: &mut [u8]) -> Result<Option<(usize, IpEndpoint)>> {
        match self.socket.recv_from(buffer).await {
            Ok((len, meta)) => {
                let endpoint = IpEndpoint::new(meta.endpoint.addr, meta.endpoint.port);
                Ok(Some((len, endpoint)))
            }
            Err(_) => Ok(None),
        }
    }

    /// Check if socket is bound
    pub fn is_bound(&self) -> bool {
        self.socket.is_open()
    }

    /// Close the socket
    pub fn close(&mut self) {
        self.socket.close();
        self.remote_endpoint = None;
    }
}

impl RawTransport for UdpTransport {
    type Error = Error;

    async fn send_bytes(&mut self, data: &[u8]) -> Result<()> {
        if let Some(endpoint) = self.remote_endpoint {
            self.socket
                .send_to(data, endpoint)
                .await
                .map_err(|_| Error::NetworkError)?;
            Ok(())
        } else {
            Err(Error::NetworkError)
        }
    }

    async fn receive_bytes(&mut self, buffer: &mut [u8]) -> Result<Option<usize>> {
        match self.socket.recv_from(buffer).await {
            Ok((len, meta)) => {
                let endpoint = IpEndpoint::new(meta.endpoint.addr, meta.endpoint.port);
                if self.remote_endpoint.is_none() {
                    self.remote_endpoint = Some(endpoint);
                }
                Ok(Some(len))
            }
            Err(_) => Ok(None),
        }
    }
}

pub struct UdpClientTransport {
    transport: UdpTransport,
}

impl UdpClientTransport {
    pub fn new(
        stack: Stack<'static>,
        server_addr: &str,
        server_port: u16,
        rx_meta: &'static mut [PacketMetadata],
        rx_buffer: &'static mut [u8],
        tx_meta: &'static mut [PacketMetadata],
        tx_buffer: &'static mut [u8],
    ) -> Result<Self> {
        let mut transport = UdpTransport::new(stack, rx_meta, rx_buffer, tx_meta, tx_buffer);
        transport.set_remote(server_addr, server_port)?;

        Ok(Self { transport })
    }

    /// Bind to specific local port
    pub fn bind(&mut self, local_port: u16) -> Result<()> {
        self.transport.bind(local_port)
    }

    /// Get inner transport
    pub fn inner_mut(&mut self) -> &mut UdpTransport {
        &mut self.transport
    }
}

impl RawTransport for UdpClientTransport {
    type Error = Error;

    async fn send_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.transport.send_bytes(data).await
    }

    async fn receive_bytes(&mut self, buffer: &mut [u8]) -> Result<Option<usize>> {
        self.transport.receive_bytes(buffer).await
    }
}

pub struct UdpServerTransport {
    transport: UdpTransport,
    current_client: Option<IpEndpoint>,
}

impl UdpServerTransport {
    pub fn new(
        stack: Stack<'static>,
        listen_port: u16,
        rx_meta: &'static mut [PacketMetadata],
        rx_buffer: &'static mut [u8],
        tx_meta: &'static mut [PacketMetadata],
        tx_buffer: &'static mut [u8],
    ) -> Result<Self> {
        let mut transport = UdpTransport::new(stack, rx_meta, rx_buffer, tx_meta, tx_buffer);
        transport.bind(listen_port)?;

        Ok(Self {
            transport,
            current_client: None,
        })
    }

    /// Wait for first client message
    pub async fn wait_for_client(&mut self) -> Result<IpEndpoint> {
        let mut buffer = [0u8; 1];

        loop {
            if let Some((_, endpoint)) = self.transport.receive_from(&mut buffer).await? {
                self.current_client = Some(endpoint);
                self.transport.set_remote_endpoint(endpoint);
                return Ok(endpoint);
            }
            embassy_time::Timer::after(embassy_time::Duration::from_millis(10)).await;
        }
    }

    /// Set current client manually
    pub fn set_current_client(&mut self, endpoint: IpEndpoint) {
        self.current_client = Some(endpoint);
        self.transport.set_remote_endpoint(endpoint);
    }

    /// Get current client endpoint
    pub fn get_current_client(&self) -> Option<IpEndpoint> {
        self.current_client
    }

    /// Send to specific client
    pub async fn send_to_client(&mut self, data: &[u8], client: IpEndpoint) -> Result<()> {
        self.transport.send_to(data, client).await
    }

    /// Get inner transport
    pub fn inner_mut(&mut self) -> &mut UdpTransport {
        &mut self.transport
    }

    /// Close the transport
    pub fn close(&mut self) {
        self.transport.close();
        self.current_client = None;
    }
}

impl RawTransport for UdpServerTransport {
    type Error = Error;

    async fn send_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.transport.send_bytes(data).await
    }

    async fn receive_bytes(&mut self, buffer: &mut [u8]) -> Result<Option<usize>> {
        match self.transport.receive_from(buffer).await? {
            Some((len, endpoint)) => {
                // Update current client if different
                if self.current_client != Some(endpoint) {
                    self.current_client = Some(endpoint);
                    self.transport.set_remote_endpoint(endpoint);
                }
                Ok(Some(len))
            }
            None => Ok(None),
        }
    }
}
