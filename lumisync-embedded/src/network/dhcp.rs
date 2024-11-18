use core::net::Ipv4Addr;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use embassy_net::udp::PacketMetadata;
use embassy_net::{IpEndpoint, Stack};
use embassy_time::{Duration, Instant};

use crate::{Error, Result};

use super::transport::UdpTransport;

const DHCP_SERVER_PORT: u16 = 67;
const DHCP_CLIENT_PORT: u16 = 68;
const DHCP_MAGIC_COOKIE: u32 = 0x63825363;

// DHCP Message Types
const DHCP_DISCOVER: u8 = 1;
const DHCP_OFFER: u8 = 2;
const DHCP_REQUEST: u8 = 3;
const DHCP_DECLINE: u8 = 4;
const DHCP_ACK: u8 = 5;
const DHCP_NAK: u8 = 6;
const DHCP_RELEASE: u8 = 7;
const DHCP_INFORM: u8 = 8;

// DHCP Options
const OPT_SUBNET_MASK: u8 = 1;
const OPT_ROUTER: u8 = 3;
const OPT_DNS_SERVERS: u8 = 6;
const OPT_REQUESTED_IP: u8 = 50;
const OPT_LEASE_TIME: u8 = 51;
const OPT_MESSAGE_TYPE: u8 = 53;
const OPT_SERVER_ID: u8 = 54;
const OPT_PARAMETER_LIST: u8 = 55;
const OPT_CLIENT_ID: u8 = 61;
const OPT_END: u8 = 255;

#[derive(Debug, Clone)]
pub struct DhcpConfig {
    pub server_ip: Ipv4Addr,
    pub subnet_mask: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub dns_server: Ipv4Addr,
    pub pool_start: Ipv4Addr,
    pub pool_end: Ipv4Addr,
    pub lease_time: u32,
}

impl Default for DhcpConfig {
    fn default() -> Self {
        Self {
            server_ip: Ipv4Addr::new(192, 168, 4, 1),
            subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
            gateway: Ipv4Addr::new(192, 168, 4, 1),
            dns_server: Ipv4Addr::new(192, 168, 4, 1),
            pool_start: Ipv4Addr::new(192, 168, 4, 10),
            pool_end: Ipv4Addr::new(192, 168, 4, 100),
            lease_time: 3600,
        }
    }
}

#[derive(Debug, Clone)]
struct DhcpLease {
    mac_address: [u8; 6],
    ip_address: Ipv4Addr,
    lease_start: Instant,
    lease_duration: Duration,
    client_id: Option<Vec<u8>>,
}

#[derive(Debug)]
struct DhcpMessage {
    op: u8,
    htype: u8,
    hlen: u8,
    hops: u8,
    xid: u32,
    secs: u16,
    flags: u16,
    ciaddr: Ipv4Addr,
    yiaddr: Ipv4Addr,
    siaddr: Ipv4Addr,
    giaddr: Ipv4Addr,
    chaddr: [u8; 16],
    sname: [u8; 64],
    file: [u8; 128],
    options: Vec<DhcpOption>,
}

#[derive(Debug, Clone)]
enum DhcpOption {
    MessageType(u8),
    SubnetMask(Ipv4Addr),
    Router(Ipv4Addr),
    DnsServers(Vec<Ipv4Addr>),
    LeaseTime(u32),
    ServerId(Ipv4Addr),
    RequestedIp(Ipv4Addr),
    ClientId(Vec<u8>),
    ParameterRequestList(Vec<u8>),
    Unknown(u8, Vec<u8>),
}

pub struct DhcpServer {
    transport: UdpTransport,
    config: DhcpConfig,
    leases: BTreeMap<[u8; 6], DhcpLease>,
    next_ip: u32,
}

impl DhcpServer {
    pub fn new(
        stack: Stack<'static>,
        config: DhcpConfig,
        rx_meta: &'static mut [PacketMetadata],
        rx_buffer: &'static mut [u8],
        tx_meta: &'static mut [PacketMetadata],
        tx_buffer: &'static mut [u8],
    ) -> Result<Self> {
        let mut transport = UdpTransport::new(stack, rx_meta, rx_buffer, tx_meta, tx_buffer);
        transport.bind(DHCP_SERVER_PORT)?;

        let next_ip = u32::from_be_bytes(config.pool_start.octets());

        Ok(Self {
            transport,
            config,
            leases: BTreeMap::new(),
            next_ip,
        })
    }

    pub async fn process(&mut self) -> Result<()> {
        let mut buffer = [0u8; 1024];

        if let Some((len, client_endpoint)) = self.transport.receive_from(&mut buffer).await? {
            log::debug!(
                "Received DHCP packet from {}: {} bytes",
                client_endpoint,
                len
            );

            if let Ok(message) = Self::parse_dhcp_message(&buffer[..len]) {
                if let Some(response) = self.handle_dhcp_message(message, client_endpoint).await? {
                    let response_data = Self::serialize_dhcp_message(&response)?;

                    // Send response to broadcast address on client port
                    let broadcast_endpoint = IpEndpoint::new(
                        embassy_net::IpAddress::v4(255, 255, 255, 255),
                        DHCP_CLIENT_PORT,
                    );

                    self.transport
                        .send_to(&response_data, broadcast_endpoint)
                        .await?;
                    log::debug!("Sent DHCP response: {} bytes", response_data.len());
                }
            }
        }

        self.cleanup_expired_leases();
        Ok(())
    }

    async fn handle_dhcp_message(
        &mut self,
        message: DhcpMessage,
        _client_endpoint: IpEndpoint,
    ) -> Result<Option<DhcpMessage>> {
        let mac_addr = self.extract_mac_address(&message);
        let message_type = self.get_message_type(&message);

        match message_type {
            Some(DHCP_DISCOVER) => {
                log::info!(
                    "DHCP DISCOVER from {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    mac_addr[0],
                    mac_addr[1],
                    mac_addr[2],
                    mac_addr[3],
                    mac_addr[4],
                    mac_addr[5]
                );
                self.handle_discover(message).await
            }
            Some(DHCP_REQUEST) => {
                log::info!(
                    "DHCP REQUEST from {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    mac_addr[0],
                    mac_addr[1],
                    mac_addr[2],
                    mac_addr[3],
                    mac_addr[4],
                    mac_addr[5]
                );
                self.handle_request(message).await
            }
            Some(DHCP_RELEASE) => {
                log::info!(
                    "DHCP RELEASE from {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    mac_addr[0],
                    mac_addr[1],
                    mac_addr[2],
                    mac_addr[3],
                    mac_addr[4],
                    mac_addr[5]
                );
                self.handle_release(message).await;
                Ok(None)
            }
            _ => {
                log::debug!("Unhandled DHCP message type: {:?}", message_type);
                Ok(None)
            }
        }
    }

    async fn handle_discover(&mut self, message: DhcpMessage) -> Result<Option<DhcpMessage>> {
        let mac_addr = self.extract_mac_address(&message);
        let offered_ip = self.find_or_allocate_ip(mac_addr)?;

        if let Some(ip) = offered_ip {
            let mut response = self.create_base_response(&message, ip);
            response.options.push(DhcpOption::MessageType(DHCP_OFFER));
            response
                .options
                .push(DhcpOption::ServerId(self.config.server_ip));
            response
                .options
                .push(DhcpOption::LeaseTime(self.config.lease_time));
            response
                .options
                .push(DhcpOption::SubnetMask(self.config.subnet_mask));
            response
                .options
                .push(DhcpOption::Router(self.config.gateway));
            response
                .options
                .push(DhcpOption::DnsServers(alloc::vec![self.config.dns_server]));

            log::info!(
                "Offering IP {} to MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                ip,
                mac_addr[0],
                mac_addr[1],
                mac_addr[2],
                mac_addr[3],
                mac_addr[4],
                mac_addr[5]
            );

            Ok(Some(response))
        } else {
            log::warn!("No IP addresses available for DHCP DISCOVER");
            Ok(None)
        }
    }

    async fn handle_request(&mut self, message: DhcpMessage) -> Result<Option<DhcpMessage>> {
        let mac_addr = self.extract_mac_address(&message);
        let requested_ip = self.get_requested_ip(&message);
        let server_id = self.get_server_id(&message);

        // Check if this request is for our server
        if let Some(sid) = server_id {
            if sid != self.config.server_ip {
                return Ok(None); // Not for us
            }
        }

        if let Some(req_ip) = requested_ip {
            if self.can_assign_ip(mac_addr, req_ip) {
                // Create lease
                let lease = DhcpLease {
                    mac_address: mac_addr,
                    ip_address: req_ip,
                    lease_start: Instant::now(),
                    lease_duration: Duration::from_secs(self.config.lease_time as u64),
                    client_id: self.get_client_id(&message),
                };

                self.leases.insert(mac_addr, lease);

                let mut response = self.create_base_response(&message, req_ip);
                response.options.push(DhcpOption::MessageType(DHCP_ACK));
                response
                    .options
                    .push(DhcpOption::ServerId(self.config.server_ip));
                response
                    .options
                    .push(DhcpOption::LeaseTime(self.config.lease_time));
                response
                    .options
                    .push(DhcpOption::SubnetMask(self.config.subnet_mask));
                response
                    .options
                    .push(DhcpOption::Router(self.config.gateway));
                response
                    .options
                    .push(DhcpOption::DnsServers(alloc::vec![self.config.dns_server]));

                log::info!(
                    "ACK: Assigned IP {} to MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    req_ip,
                    mac_addr[0],
                    mac_addr[1],
                    mac_addr[2],
                    mac_addr[3],
                    mac_addr[4],
                    mac_addr[5]
                );

                Ok(Some(response))
            } else {
                // Send NAK
                let mut response = self.create_base_response(&message, Ipv4Addr::UNSPECIFIED);
                response.options.push(DhcpOption::MessageType(DHCP_NAK));
                response
                    .options
                    .push(DhcpOption::ServerId(self.config.server_ip));

                log::warn!(
                    "NAK: Cannot assign IP {} to MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    req_ip,
                    mac_addr[0],
                    mac_addr[1],
                    mac_addr[2],
                    mac_addr[3],
                    mac_addr[4],
                    mac_addr[5]
                );

                Ok(Some(response))
            }
        } else {
            Ok(None)
        }
    }

    async fn handle_release(&mut self, message: DhcpMessage) {
        let mac_addr = self.extract_mac_address(&message);
        if let Some(lease) = self.leases.remove(&mac_addr) {
            log::info!(
                "Released IP {} from MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                lease.ip_address,
                mac_addr[0],
                mac_addr[1],
                mac_addr[2],
                mac_addr[3],
                mac_addr[4],
                mac_addr[5]
            );
        }
    }

    fn find_or_allocate_ip(&mut self, mac_addr: [u8; 6]) -> Result<Option<Ipv4Addr>> {
        // Check for existing lease
        if let Some(lease) = self.leases.get(&mac_addr) {
            let now = Instant::now();
            if now.saturating_duration_since(lease.lease_start) < lease.lease_duration {
                return Ok(Some(lease.ip_address));
            }
        }

        // Try to find a free IP
        let pool_start = u32::from_be_bytes(self.config.pool_start.octets());
        let pool_end = u32::from_be_bytes(self.config.pool_end.octets());

        for ip_u32 in pool_start..=pool_end {
            let ip = Ipv4Addr::from(ip_u32.to_be_bytes());
            if !self.is_ip_allocated(ip) {
                return Ok(Some(ip));
            }
        }

        Ok(None)
    }

    fn can_assign_ip(&self, mac_addr: [u8; 6], ip: Ipv4Addr) -> bool {
        let pool_start = u32::from_be_bytes(self.config.pool_start.octets());
        let pool_end = u32::from_be_bytes(self.config.pool_end.octets());
        let ip_u32 = u32::from_be_bytes(ip.octets());

        if ip_u32 < pool_start || ip_u32 > pool_end {
            return false;
        }

        // Check if IP is already allocated to someone else
        for (mac, lease) in &self.leases {
            if *mac != mac_addr && lease.ip_address == ip {
                let now = Instant::now();
                if now.saturating_duration_since(lease.lease_start) < lease.lease_duration {
                    return false;
                }
            }
        }

        true
    }

    fn is_ip_allocated(&self, ip: Ipv4Addr) -> bool {
        let now = Instant::now();
        self.leases.values().any(|lease| {
            lease.ip_address == ip
                && now.saturating_duration_since(lease.lease_start) < lease.lease_duration
        })
    }

    fn cleanup_expired_leases(&mut self) {
        let now = Instant::now();
        self.leases.retain(|_, lease| {
            now.saturating_duration_since(lease.lease_start) < lease.lease_duration
        });
    }

    fn create_base_response(&self, request: &DhcpMessage, yiaddr: Ipv4Addr) -> DhcpMessage {
        DhcpMessage {
            op: 2, // BOOTREPLY
            htype: request.htype,
            hlen: request.hlen,
            hops: 0,
            xid: request.xid,
            secs: 0,
            flags: request.flags,
            ciaddr: Ipv4Addr::UNSPECIFIED,
            yiaddr,
            siaddr: self.config.server_ip,
            giaddr: request.giaddr,
            chaddr: request.chaddr,
            sname: [0; 64],
            file: [0; 128],
            options: Vec::new(),
        }
    }

    fn extract_mac_address(&self, message: &DhcpMessage) -> [u8; 6] {
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&message.chaddr[..6]);
        mac
    }

    fn get_message_type(&self, message: &DhcpMessage) -> Option<u8> {
        for option in &message.options {
            if let DhcpOption::MessageType(msg_type) = option {
                return Some(*msg_type);
            }
        }
        None
    }

    fn get_requested_ip(&self, message: &DhcpMessage) -> Option<Ipv4Addr> {
        for option in &message.options {
            if let DhcpOption::RequestedIp(ip) = option {
                return Some(*ip);
            }
        }
        None
    }

    fn get_server_id(&self, message: &DhcpMessage) -> Option<Ipv4Addr> {
        for option in &message.options {
            if let DhcpOption::ServerId(ip) = option {
                return Some(*ip);
            }
        }
        None
    }

    fn get_client_id(&self, message: &DhcpMessage) -> Option<Vec<u8>> {
        for option in &message.options {
            if let DhcpOption::ClientId(id) = option {
                return Some(id.clone());
            }
        }
        None
    }

    fn parse_dhcp_message(data: &[u8]) -> Result<DhcpMessage> {
        if data.len() < 240 {
            return Err(Error::SerializationError);
        }

        let op = data[0];
        let htype = data[1];
        let hlen = data[2];
        let hops = data[3];
        let xid = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let secs = u16::from_be_bytes([data[8], data[9]]);
        let flags = u16::from_be_bytes([data[10], data[11]]);

        let ciaddr = Ipv4Addr::new(data[12], data[13], data[14], data[15]);
        let yiaddr = Ipv4Addr::new(data[16], data[17], data[18], data[19]);
        let siaddr = Ipv4Addr::new(data[20], data[21], data[22], data[23]);
        let giaddr = Ipv4Addr::new(data[24], data[25], data[26], data[27]);

        let mut chaddr = [0u8; 16];
        chaddr.copy_from_slice(&data[28..44]);

        let mut sname = [0u8; 64];
        sname.copy_from_slice(&data[44..108]);

        let mut file = [0u8; 128];
        file.copy_from_slice(&data[108..236]);

        // Check magic cookie
        let magic = u32::from_be_bytes([data[236], data[237], data[238], data[239]]);
        if magic != DHCP_MAGIC_COOKIE {
            return Err(Error::SerializationError);
        }

        let options = Self::parse_options(&data[240..])?;

        Ok(DhcpMessage {
            op,
            htype,
            hlen,
            hops,
            xid,
            secs,
            flags,
            ciaddr,
            yiaddr,
            siaddr,
            giaddr,
            chaddr,
            sname,
            file,
            options,
        })
    }

    fn parse_options(data: &[u8]) -> Result<Vec<DhcpOption>> {
        let mut options = Vec::new();
        let mut i = 0;

        while i < data.len() {
            let option_type = data[i];
            i += 1;

            if option_type == OPT_END {
                break;
            }

            if i >= data.len() {
                break;
            }

            let length = data[i] as usize;
            i += 1;

            if i + length > data.len() {
                break;
            }

            let option_data = &data[i..i + length];
            i += length;

            let option = match option_type {
                OPT_MESSAGE_TYPE if length == 1 => DhcpOption::MessageType(option_data[0]),
                OPT_SUBNET_MASK if length == 4 => DhcpOption::SubnetMask(Ipv4Addr::new(
                    option_data[0],
                    option_data[1],
                    option_data[2],
                    option_data[3],
                )),
                OPT_ROUTER if length == 4 => DhcpOption::Router(Ipv4Addr::new(
                    option_data[0],
                    option_data[1],
                    option_data[2],
                    option_data[3],
                )),
                OPT_REQUESTED_IP if length == 4 => DhcpOption::RequestedIp(Ipv4Addr::new(
                    option_data[0],
                    option_data[1],
                    option_data[2],
                    option_data[3],
                )),
                OPT_LEASE_TIME if length == 4 => DhcpOption::LeaseTime(u32::from_be_bytes([
                    option_data[0],
                    option_data[1],
                    option_data[2],
                    option_data[3],
                ])),
                OPT_SERVER_ID if length == 4 => DhcpOption::ServerId(Ipv4Addr::new(
                    option_data[0],
                    option_data[1],
                    option_data[2],
                    option_data[3],
                )),
                OPT_CLIENT_ID => DhcpOption::ClientId(option_data.to_vec()),
                OPT_PARAMETER_LIST => DhcpOption::ParameterRequestList(option_data.to_vec()),
                _ => DhcpOption::Unknown(option_type, option_data.to_vec()),
            };

            options.push(option);
        }

        Ok(options)
    }

    fn serialize_dhcp_message(message: &DhcpMessage) -> Result<Vec<u8>> {
        let mut data = Vec::with_capacity(576);

        // Fixed fields
        data.push(message.op);
        data.push(message.htype);
        data.push(message.hlen);
        data.push(message.hops);
        data.extend_from_slice(&message.xid.to_be_bytes());
        data.extend_from_slice(&message.secs.to_be_bytes());
        data.extend_from_slice(&message.flags.to_be_bytes());
        data.extend_from_slice(&message.ciaddr.octets());
        data.extend_from_slice(&message.yiaddr.octets());
        data.extend_from_slice(&message.siaddr.octets());
        data.extend_from_slice(&message.giaddr.octets());
        data.extend_from_slice(&message.chaddr);
        data.extend_from_slice(&message.sname);
        data.extend_from_slice(&message.file);

        // Magic cookie
        data.extend_from_slice(&DHCP_MAGIC_COOKIE.to_be_bytes());

        // Options
        for option in &message.options {
            match option {
                DhcpOption::MessageType(msg_type) => {
                    data.push(OPT_MESSAGE_TYPE);
                    data.push(1);
                    data.push(*msg_type);
                }
                DhcpOption::SubnetMask(ip) => {
                    data.push(OPT_SUBNET_MASK);
                    data.push(4);
                    data.extend_from_slice(&ip.octets());
                }
                DhcpOption::Router(ip) => {
                    data.push(OPT_ROUTER);
                    data.push(4);
                    data.extend_from_slice(&ip.octets());
                }
                DhcpOption::DnsServers(servers) => {
                    data.push(OPT_DNS_SERVERS);
                    data.push((servers.len() * 4) as u8);
                    for server in servers {
                        data.extend_from_slice(&server.octets());
                    }
                }
                DhcpOption::LeaseTime(time) => {
                    data.push(OPT_LEASE_TIME);
                    data.push(4);
                    data.extend_from_slice(&time.to_be_bytes());
                }
                DhcpOption::ServerId(ip) => {
                    data.push(OPT_SERVER_ID);
                    data.push(4);
                    data.extend_from_slice(&ip.octets());
                }
                DhcpOption::RequestedIp(ip) => {
                    data.push(OPT_REQUESTED_IP);
                    data.push(4);
                    data.extend_from_slice(&ip.octets());
                }
                DhcpOption::ClientId(id) => {
                    data.push(OPT_CLIENT_ID);
                    data.push(id.len() as u8);
                    data.extend_from_slice(id);
                }
                DhcpOption::ParameterRequestList(params) => {
                    data.push(OPT_PARAMETER_LIST);
                    data.push(params.len() as u8);
                    data.extend_from_slice(params);
                }
                DhcpOption::Unknown(opt_type, opt_data) => {
                    data.push(*opt_type);
                    data.push(opt_data.len() as u8);
                    data.extend_from_slice(opt_data);
                }
            }
        }

        // End option
        data.push(OPT_END);

        // Pad to minimum size if needed
        while data.len() < 300 {
            data.push(0);
        }

        Ok(data)
    }

    pub fn get_active_leases(&self) -> Vec<(Ipv4Addr, [u8; 6])> {
        let now = Instant::now();
        self.leases
            .values()
            .filter(|lease| now.saturating_duration_since(lease.lease_start) < lease.lease_duration)
            .map(|lease| (lease.ip_address, lease.mac_address))
            .collect()
    }

    pub fn config(&self) -> &DhcpConfig {
        &self.config
    }
}
