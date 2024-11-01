use alloc::format;

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Timer};
use trouble_host::prelude::*;

use crate::{Error, Result as InternalResult};

use super::RawTransport;

/// BLE Characteristic UUID for message exchange
const MESSAGE_CHAR_UUID: Uuid = Uuid::new_short(0xABCE);

struct BleMessage {
    data: [u8; 512],
    len: usize,
}

#[gatt_server]
struct BleServer {
    message_service: MessageService,
}

#[gatt_service(uuid = "12345678-1234-1234-1234-123456789abc")]
struct MessageService {
    #[characteristic(uuid = MESSAGE_CHAR_UUID, read, write, notify, value = [0u8; 512])]
    message: [u8; 512],
}

/// BLE Peripheral transport
pub struct BlePeripheralTransport {
    rx_channel: Channel<CriticalSectionRawMutex, BleMessage, 4>,
    tx_channel: Channel<CriticalSectionRawMutex, BleMessage, 4>,
    is_connected: bool,
}

impl BlePeripheralTransport {
    pub fn new() -> Self {
        Self {
            rx_channel: Channel::new(),
            tx_channel: Channel::new(),
            is_connected: false,
        }
    }

    /// Run the BLE peripheral and handle connections
    pub async fn run<C: Controller>(
        &mut self,
        peripheral: &mut Peripheral<'static, C, DefaultPacketPool>,
        name: &str,
    ) -> InternalResult<()> {
        loop {
            match self.advertise_and_serve(peripheral, name).await {
                Ok(()) => {
                    log::info!("BLE connection closed, restarting advertising...");
                    self.is_connected = false;
                }
                Err(e) => {
                    log::warn!("BLE peripheral error: {:?}", e);
                    self.is_connected = false;
                    Timer::after(Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn advertise_and_serve<C: Controller>(
        &mut self,
        peripheral: &mut Peripheral<'static, C, DefaultPacketPool>,
        name: &str,
    ) -> InternalResult<()> {
        // Create GATT server
        let server = BleServer::new_with_config(GapConfig::Peripheral(PeripheralConfig {
            name,
            appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
        }))
        .map_err(|_| Error::InitializationError)?;

        // Prepare advertising data
        let mut adv_data = [0; 31];
        let len = AdStructure::encode_slice(
            &[
                AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                AdStructure::ServiceUuids128(&[[
                    0x12, 0x34, 0x56, 0x78, 0x12, 0x34, 0x12, 0x34, 0x12, 0x34, 0x12, 0x34, 0x56,
                    0x78, 0x9a, 0xbc,
                ]]),
                AdStructure::CompleteLocalName(name.as_bytes()),
            ],
            &mut adv_data[..],
        )
        .map_err(|_| Error::InitializationError)?;

        // Start advertising
        log::info!("Starting BLE advertising...");
        let advertiser = peripheral
            .advertise(
                &Default::default(),
                Advertisement::ConnectableScannableUndirected {
                    adv_data: &adv_data[..len],
                    scan_data: &[],
                },
            )
            .await
            .map_err(|_| Error::NetworkError)?;

        // Accept connection
        let conn = advertiser
            .accept()
            .await
            .map_err(|_| Error::NetworkError)?
            .with_attribute_server(&server)
            .map_err(|_| Error::NetworkError)?;

        log::info!("BLE connection established");
        self.is_connected = true;

        // Handle connection events with TX handling
        let message_handle = server.message_service.message;
        loop {
            // Try to send pending messages
            if let Ok(msg) = self.tx_channel.try_receive() {
                // Update characteristic value and notify
                let mut char_data = [0u8; 512];
                char_data[..msg.len].copy_from_slice(&msg.data[..msg.len]);
                let _ = server.set(&message_handle, &char_data);

                if message_handle.notify(&conn, &char_data).await.is_err() {
                    break; // Connection lost
                }
            }

            // Handle incoming GATT events
            match conn.next().await {
                GattConnectionEvent::Disconnected { reason } => {
                    log::info!("BLE connection closed: {:?}", reason);
                    break;
                }
                GattConnectionEvent::Gatt { event: Ok(event) } => {
                    match &event {
                        GattEvent::Write(write_event) => {
                            if write_event.handle() == message_handle.handle {
                                let data = write_event.data();
                                let mut msg = BleMessage {
                                    data: [0; 512],
                                    len: data.len().min(512),
                                };
                                msg.data[..msg.len].copy_from_slice(&data[..msg.len]);
                                let _ = self.rx_channel.try_send(msg);
                            }
                        }
                        _ => {}
                    }

                    // Accept the event
                    match event.accept() {
                        Ok(reply) => reply.send().await,
                        Err(_) => break, // Connection error
                    }
                }
                GattConnectionEvent::Gatt { event: Err(_) } => {
                    break; // GATT error
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Send device status report
    pub async fn send_status_report(
        &mut self,
        device_id: u32,
        battery_level: u8,
    ) -> InternalResult<()> {
        if !self.is_connected {
            return Err(Error::NotConnected);
        }

        // Create status report message
        let status_data = format!("STATUS:ID={},BATTERY={}", device_id, battery_level);
        let status_bytes = status_data.as_bytes();

        let mut msg = BleMessage {
            data: [0; 512],
            len: status_bytes.len().min(512),
        };
        msg.data[..msg.len].copy_from_slice(&status_bytes[..msg.len]);

        self.tx_channel.send(msg).await;
        Ok(())
    }
}

impl RawTransport for BlePeripheralTransport {
    type Error = Error;

    async fn send_bytes(&mut self, data: &[u8]) -> InternalResult<()> {
        if !self.is_connected {
            return Err(Error::NotConnected);
        }

        let mut msg = BleMessage {
            data: [0; 512],
            len: data.len().min(512),
        };
        msg.data[..msg.len].copy_from_slice(&data[..msg.len]);

        self.tx_channel.send(msg).await;
        Ok(())
    }

    async fn receive_bytes(&mut self, buffer: &mut [u8]) -> InternalResult<Option<usize>> {
        if let Ok(msg) = self.rx_channel.try_receive() {
            let len = msg.len.min(buffer.len());
            buffer[..len].copy_from_slice(&msg.data[..len]);
            Ok(Some(len))
        } else {
            Ok(None)
        }
    }
}
