use alloc::vec::Vec;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use trouble_host::prelude::*;

use crate::{Error, Result as InternalResult};

use super::RawTransport;

/// BLE Service UUID for LumiSync
const LUMISYNC_SERVICE_UUID: Uuid = Uuid::new_short(0xABCD);

/// BLE Characteristic UUID for message exchange
const MESSAGE_CHAR_UUID: Uuid = Uuid::new_short(0xABCE);

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;

/// Max number of L2CAP channels
const L2CAP_CHANNELS_MAX: usize = 3;

pub struct BleCentralTransport {
    rx_channel: Channel<CriticalSectionRawMutex, Vec<u8>, 4>,
    tx_channel: Channel<CriticalSectionRawMutex, Vec<u8>, 4>,
}

impl BleCentralTransport {
    pub fn new() -> Self {
        Self {
            rx_channel: Channel::new(),
            tx_channel: Channel::new(),
        }
    }

    pub async fn connect_and_run<C: Controller>(
        &self,
        controller: C,
        target_mac: [u8; 6],
        our_address: Option<[u8; 6]>,
    ) -> InternalResult<()> {
        let address = if let Some(addr) = our_address {
            Address::random(addr)
        } else {
            Address::random([0xff, 0x8f, 0x1b, 0x05, 0xe4, 0xff])
        };

        log::info!("Initializing BLE stack with address {:?}", address);

        let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
            HostResources::new();
        let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
        let Host {
            mut central,
            mut runner,
            ..
        } = stack.build();

        let target = Address::random(target_mac);
        let config = ConnectConfig {
            connect_params: Default::default(),
            scan_config: ScanConfig {
                filter_accept_list: &[(target.kind, &target.addr)],
                ..Default::default()
            },
        };

        log::info!("Starting BLE operations...");

        let (_, comm_result) = embassy_futures::join::join(
            runner.run(),
            self.communication_task(&mut central, &target, &config, &stack),
        )
        .await;

        comm_result
    }

    async fn communication_task<'a, C: Controller, P: PacketPool>(
        &self,
        central: &mut Central<'a, C, P>,
        target: &Address,
        config: &ConnectConfig<'a>,
        stack: &'a Stack<'a, C, P>,
    ) -> InternalResult<()> {
        log::info!("Connecting to device {:?}...", target);
        let conn = central
            .connect(config)
            .await
            .map_err(|_| Error::NetworkError)?;

        log::info!("Connected, creating GATT client...");
        let client = GattClient::<C, P, 10>::new(stack, &conn)
            .await
            .map_err(|_| Error::NetworkError)?;

        let (_, comm_result) =
            embassy_futures::join::join(client.task(), self.handle_communication(&client)).await;

        comm_result
    }

    async fn handle_communication<C: Controller, P: PacketPool>(
        &self,
        client: &GattClient<'_, C, P, 10>,
    ) -> InternalResult<()> {
        // Find LumiSync service
        log::info!("Looking for LumiSync service...");
        let services = client
            .services_by_uuid(&LUMISYNC_SERVICE_UUID)
            .await
            .map_err(|_| Error::NetworkError)?;

        let service = services.first().ok_or(Error::DeviceNotFound)?;

        // Find message characteristic
        log::info!("Looking for message characteristic...");
        let message_char: Characteristic<[u8; 512]> = client
            .characteristic_by_uuid(service, &MESSAGE_CHAR_UUID)
            .await
            .map_err(|_| Error::NetworkError)?;

        // Subscribe to notifications
        log::info!("Subscribing to notifications...");
        let mut listener = client
            .subscribe(&message_char, false)
            .await
            .map_err(|_| Error::NetworkError)?;

        // Handle bidirectional communication
        let mut should_continue = true;
        while should_continue {
            let notification_task = async {
                let data = listener.next().await;
                let bytes = data.as_ref();
                let _ = self.rx_channel.try_send(bytes.to_vec());
                true // Continue
            };

            let write_task = async {
                if let Ok(tx_data) = self.tx_channel.try_receive() {
                    // Write to characteristic
                    let mut write_data = [0u8; 512];
                    let len = tx_data.len().min(512);
                    write_data[..len].copy_from_slice(&tx_data[..len]);

                    if let Err(_) = client
                        .write_characteristic(&message_char, &write_data)
                        .await
                    {
                        log::warn!("Failed to write characteristic");
                        return false; // Stop loop
                    }
                }
                // Small delay to prevent busy loop
                embassy_time::Timer::after(embassy_time::Duration::from_millis(10)).await;
                true // Continue
            };

            // Use select to run either task and get the result
            let result = embassy_futures::select::select(notification_task, write_task).await;
            should_continue = match result {
                embassy_futures::select::Either::First(continue_flag) => continue_flag,
                embassy_futures::select::Either::Second(continue_flag) => continue_flag,
            };
        }

        Ok(())
    }
}

impl RawTransport for BleCentralTransport {
    type Error = Error;

    async fn send_bytes(&mut self, data: &[u8]) -> InternalResult<()> {
        self.tx_channel.send(data.to_vec()).await;
        Ok(())
    }

    async fn receive_bytes(&mut self, buffer: &mut [u8]) -> InternalResult<Option<usize>> {
        if let Ok(data) = self.rx_channel.try_receive() {
            let len = data.len().min(buffer.len());
            buffer[..len].copy_from_slice(&data[..len]);
            Ok(Some(len))
        } else {
            Ok(None)
        }
    }
}
