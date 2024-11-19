use std::collections::{HashSet, VecDeque};
use std::ffi::c_void;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use time::OffsetDateTime;
use windows::Win32::Foundation::*;
use windows::Win32::NetworkManagement::WiFi::*;
use windows::Win32::Security::*;
use windows::Win32::System::Threading::*;
use windows::core::{GUID, PCWSTR, PWSTR};

use crate::error::{Error, Result};

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
enum WifiEvent {
    ScanComplete,
    ScanFail,
    ConnectionAttemptFail,
    ConnectionComplete,
    ConnectionStart,
    Disconnected,
    Disconnecting,
    Other(u32),
}

struct NotificationManager {
    handle: HANDLE,
    context_ptr: *mut c_void,
    event_queue: Arc<Mutex<VecDeque<WifiEvent>>>,
}

impl NotificationManager {
    fn new(handle: HANDLE) -> Result<Self> {
        let event_queue = Arc::new(Mutex::new(VecDeque::new()));

        unsafe extern "system" fn notification_callback(
            data: *mut L2_NOTIFICATION_DATA,
            context: *mut c_void,
        ) {
            if data.is_null() || context.is_null() {
                return;
            }

            let notification = unsafe { &*data };
            let queue_ptr = context as *const Arc<Mutex<VecDeque<WifiEvent>>>;
            let queue_arc = unsafe { &*queue_ptr };

            if notification.NotificationSource == WLAN_NOTIFICATION_SOURCE_ACM {
                if let Ok(mut queue) = queue_arc.lock() {
                    let code = WLAN_NOTIFICATION_ACM(notification.NotificationCode as i32);
                    let event = match code {
                        WLAN_NOTIFICATION_ACM(7) => WifiEvent::ScanComplete,
                        WLAN_NOTIFICATION_ACM(8) => WifiEvent::ScanFail,
                        WLAN_NOTIFICATION_ACM(11) => WifiEvent::ConnectionAttemptFail,
                        WLAN_NOTIFICATION_ACM(10) => WifiEvent::ConnectionComplete,
                        WLAN_NOTIFICATION_ACM(9) => WifiEvent::ConnectionStart,
                        WLAN_NOTIFICATION_ACM(21) => WifiEvent::Disconnected,
                        WLAN_NOTIFICATION_ACM(20) => WifiEvent::Disconnecting,
                        _ => WifiEvent::Other(notification.NotificationCode),
                    };
                    queue.push_back(event);

                    if queue.len() > 100 {
                        queue.pop_front();
                    }
                }
            }
        }

        let context_ptr = Box::into_raw(Box::new(event_queue.clone())) as *mut c_void;
        let mut prev_source = 0u32;
        let result = unsafe {
            WlanRegisterNotification(
                handle,
                WLAN_NOTIFICATION_SOURCE_ACM,
                true,
                Some(notification_callback),
                Some(context_ptr),
                None,
                Some(&mut prev_source),
            )
        };

        if result != ERROR_SUCCESS.0 {
            unsafe {
                let _ = Box::from_raw(context_ptr as *mut Arc<Mutex<VecDeque<WifiEvent>>>);
            }
            return Err(Error::wifi(format!(
                "Failed to register notification: {}",
                result
            )));
        }

        Ok(Self {
            handle,
            context_ptr,
            event_queue,
        })
    }

    fn wait_for_event(&self, expected: &[WifiEvent], timeout: Duration) -> Result<WifiEvent> {
        let start = Instant::now();
        let expected_discriminants = expected
            .iter()
            .map(std::mem::discriminant)
            .collect::<HashSet<_>>();

        while start.elapsed() < timeout {
            if let Ok(mut queue) = self.event_queue.lock() {
                let mut found_index = None;
                for (index, event) in queue.iter().enumerate() {
                    if expected_discriminants.contains(&std::mem::discriminant(event)) {
                        found_index = Some(index);
                        break;
                    }
                }

                if let Some(index) = found_index {
                    return Ok(queue.remove(index).unwrap());
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        Err(Error::connection(format!(
            "Timeout waiting for expected events: {expected:?}"
        )))
    }
}

impl Drop for NotificationManager {
    fn drop(&mut self) {
        unsafe {
            WlanRegisterNotification(
                self.handle,
                WLAN_NOTIFICATION_SOURCE_NONE,
                true,
                None,
                None,
                None,
                None,
            );

            if !self.context_ptr.is_null() {
                let _ = Box::from_raw(self.context_ptr as *mut Arc<Mutex<VecDeque<WifiEvent>>>);
                self.context_ptr = ptr::null_mut();
            }
        }
    }
}

struct WifiHandle(HANDLE);

unsafe impl Send for WifiHandle {}
unsafe impl Sync for WifiHandle {}

impl Drop for WifiHandle {
    fn drop(&mut self) {
        unsafe {
            WlanCloseHandle(self.0, None);
        }
    }
}

#[derive(Clone)]
pub struct Backend {
    client_handle: Option<Arc<WifiHandle>>,
}

impl Default for Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend {
    pub fn new() -> Self {
        Self {
            client_handle: None,
        }
    }

    fn ensure_initialized(&mut self) -> Result<(HANDLE, GUID)> {
        let handle = self.ensure_client_handle()?;
        let interface = self.get_first_interface(handle)?;
        Ok((handle, interface))
    }

    fn ensure_client_handle(&mut self) -> Result<HANDLE> {
        if let Some(handle_arc) = &self.client_handle {
            if !handle_arc.0.is_invalid() {
                return Ok(handle_arc.0);
            }
        }

        let mut client_handle = HANDLE::default();
        let mut negotiated_version = 0u32;

        let result =
            unsafe { WlanOpenHandle(2, None, &mut negotiated_version, &mut client_handle) };

        if result != ERROR_SUCCESS.0 {
            return Err(Error::wifi(format!(
                "Failed to open WiFi handle: {}",
                result
            )));
        }

        self.client_handle = Some(Arc::new(WifiHandle(client_handle)));
        Ok(client_handle)
    }

    fn get_first_interface(&self, client_handle: HANDLE) -> Result<GUID> {
        let mut interface_list: *mut WLAN_INTERFACE_INFO_LIST = ptr::null_mut();

        let result = unsafe { WlanEnumInterfaces(client_handle, None, &mut interface_list) };

        if result != ERROR_SUCCESS.0 {
            return Err(Error::wifi(format!(
                "Failed to enumerate wireless interfaces: {}",
                result
            )));
        }

        let interface_guid = unsafe {
            if interface_list.is_null() {
                return Err(Error::wifi("Interface list is null pointer".to_string()));
            }
            if (*interface_list).dwNumberOfItems == 0 {
                WlanFreeMemory(interface_list as *mut c_void);
                return Err(Error::not_found("No wireless interfaces found".to_string()));
            }

            let interface_info = &(*interface_list).InterfaceInfo[0];
            let guid = interface_info.InterfaceGuid;
            WlanFreeMemory(interface_list as *mut c_void);
            guid
        };

        Ok(interface_guid)
    }

    fn to_wide(&self, s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn parse_channel_info(&self, freq_khz: u32) -> (u32, Band) {
        let freq_mhz = freq_khz / 1000;
        let channel = match freq_mhz {
            2412..=2484 => (freq_mhz - 2412) / 5 + 1,
            5170..=5825 => (freq_mhz - 5000) / 5,
            5955..=7115 => (freq_mhz - 5955) / 5 + 1,
            _ => 0,
        };

        let band = match channel {
            ch if (1..=233).contains(&ch) && ch % 4 == 1 => Band::GHz6,
            36..=177 => Band::GHz5,
            1..=14 => Band::GHz2,
            _ => Band::Unknown,
        };

        (channel, band)
    }

    fn infer_security(&self, capability: u16) -> Security {
        if capability & 0x0010 != 0 {
            Security::Wpa2Personal
        } else {
            Security::Open
        }
    }

    fn create_profile_xml(&self, creds: &Credentials) -> String {
        let (auth, encryption) = match creds.security {
            Security::Open => ("open", "none"),
            Security::Wep => ("WEP", "WEP"),
            Security::WpaPersonal => ("WPA", "AES"),
            Security::Wpa2Personal => ("WPA2PSK", "AES"),
            Security::Wpa3Personal => ("WPA3SAE", "AES"),
            _ => ("WPA2PSK", "AES"),
        };

        let key_material = creds.passphrase.as_deref().unwrap_or("");

        format!(
            r#"<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
    <name>{}</name>
    <SSIDConfig><SSID><name>{}</name></SSID></SSIDConfig>
    <connectionType>ESS</connectionType>
    <connectionMode>auto</connectionMode>
    <MSM>
        <security>
            <authEncryption>
                <authentication>{}</authentication>
                <encryption>{}</encryption>
            </authEncryption>
            <sharedKey>
                <keyType>passPhrase</keyType>
                <protected>false</protected>
                <keyMaterial>{}</keyMaterial>
            </sharedKey>
        </security>
    </MSM>
</WLANProfile>"#,
            creds.ssid.0, creds.ssid.0, auth, encryption, key_material
        )
    }

    fn extract_credentials_from_xml(&self, xml: &str) -> (Security, Option<String>) {
        let security = if xml.contains("<authentication>open</authentication>") {
            Security::Open
        } else if xml.contains("<authentication>WEP</authentication>") {
            Security::Wep
        } else if xml.contains("<authentication>WPA</authentication>") {
            Security::WpaPersonal
        } else if xml.contains("<authentication>WPA2PSK</authentication>") {
            Security::Wpa2Personal
        } else if xml.contains("<authentication>WPA3SAE</authentication>") {
            Security::Wpa3Personal
        } else if xml.contains("<authentication>WPA2</authentication>") {
            Security::Wpa2Enterprise
        } else if xml.contains("<authentication>WPA3</authentication>") {
            Security::Wpa3Enterprise
        } else {
            Security::Unknown
        };

        let passphrase = if let Some(start) = xml.find("<keyMaterial>") {
            if let Some(end) = xml[start..].find("</keyMaterial>") {
                let key_start = start + "<keyMaterial>".len();
                let key_end = start + end;
                let password = xml[key_start..key_end].trim();
                if password.is_empty() {
                    None
                } else {
                    Some(password.to_string())
                }
            } else {
                None
            }
        } else {
            None
        };

        (security, passphrase)
    }

    fn current_connection_sync(&mut self) -> Result<Option<ConnectionInfo>> {
        let (handle, interface) = self.ensure_initialized()?;

        let mut data_size = 0u32;
        let mut connection_attrs: *mut WLAN_CONNECTION_ATTRIBUTES = ptr::null_mut();

        let result = unsafe {
            WlanQueryInterface(
                handle,
                &interface,
                wlan_intf_opcode_current_connection,
                None,
                &mut data_size,
                &mut connection_attrs as *mut _ as *mut *mut c_void,
                None,
            )
        };

        if result != ERROR_SUCCESS.0 {
            let state = match result {
                0x80070015 | 0x80070002 => ConnState::Disconnected,
                _ => ConnState::Error(format!("Query failed: {}", result)),
            };

            return Ok(Some(ConnectionInfo {
                state,
                ssid: None,
                access_point: None,
                ip_address: None,
                gateway: None,
                dns_servers: vec![],
                speed_mbps: None,
                since: None,
            }));
        }

        let connection_info = unsafe {
            if connection_attrs.is_null() {
                return Ok(Some(ConnectionInfo {
                    state: ConnState::Error("Connection attributes pointer is null".to_string()),
                    ssid: None,
                    access_point: None,
                    ip_address: None,
                    gateway: None,
                    dns_servers: vec![],
                    speed_mbps: None,
                    since: None,
                }));
            }
            let attrs = &*connection_attrs;

            let ssid = if attrs.wlanAssociationAttributes.dot11Ssid.uSSIDLength > 0 {
                let ssid_bytes = &attrs.wlanAssociationAttributes.dot11Ssid.ucSSID
                    [..attrs.wlanAssociationAttributes.dot11Ssid.uSSIDLength as usize];
                Some(Ssid(String::from_utf8_lossy(ssid_bytes).to_string()))
            } else {
                None
            };

            let mut bssid_bytes = [0u8; 6];
            bssid_bytes.copy_from_slice(&attrs.wlanAssociationAttributes.dot11Bssid);
            let bssid = Some(Bssid(bssid_bytes));

            #[allow(non_upper_case_globals)]
            let state = match attrs.isState {
                wlan_interface_state_connected => ConnState::Connected,
                wlan_interface_state_authenticating => ConnState::Authenticating,
                _ => ConnState::Disconnected,
            };

            let speed_mbps = if attrs.wlanAssociationAttributes.ulRxRate > 0 {
                Some(attrs.wlanAssociationAttributes.ulRxRate / 1000)
            } else {
                None
            };

            WlanFreeMemory(connection_attrs as *mut c_void);

            ConnectionInfo {
                state,
                ssid,
                access_point: bssid,
                ip_address: None,
                gateway: None,
                dns_servers: vec![],
                speed_mbps,
                since: Some(OffsetDateTime::now_utc()),
            }
        };

        Ok(Some(connection_info))
    }

    fn has_admin_privileges(&self) -> bool {
        unsafe {
            let mut token: HANDLE = HANDLE::default();

            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
                return false;
            }

            let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
            let mut return_length = 0u32;

            let result = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut c_void),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut return_length,
            );

            let _ = CloseHandle(token);

            result.is_ok() && elevation.TokenIsElevated != 0
        }
    }
}

#[async_trait::async_trait]
impl WifiBackend for Backend {
    async fn scan(&self) -> Result<Vec<Network>> {
        let backend = self.clone();

        tokio::task::spawn_blocking(move || {
            let mut backend = backend;
            let (handle, interface) = backend.ensure_initialized()?;

            let notification_mgr = NotificationManager::new(handle)?;

            let result = unsafe { WlanScan(handle, &interface, None, None, None) };
            if result != ERROR_SUCCESS.0 {
                return Err(Error::wifi(format!("Scan failed: {}", result)));
            }

            notification_mgr.wait_for_event(&[WifiEvent::ScanComplete], Duration::from_secs(10))?;

            let mut bss_list: *mut WLAN_BSS_LIST = ptr::null_mut();
            let result = unsafe {
                WlanGetNetworkBssList(
                    handle,
                    &interface,
                    None,
                    dot11_BSS_type_any,
                    false,
                    None,
                    &mut bss_list,
                )
            };

            if result != ERROR_SUCCESS.0 {
                return Err(Error::wifi(if result == 0x80070005 {
                    "Access denied. Please enable location services on Windows 11".to_string()
                } else {
                    format!("Failed to get network list: {}", result)
                }));
            }

            let networks = unsafe {
                if bss_list.is_null() {
                    return Err(Error::wifi("BSS list is null pointer".to_string()));
                }
                let bss_count = (*bss_list).dwNumberOfItems as usize;
                let bss_entries =
                    std::slice::from_raw_parts((*bss_list).wlanBssEntries.as_ptr(), bss_count);
                let mut network_map = std::collections::HashMap::<String, Network>::new();

                for bss in bss_entries {
                    if bss.dot11Ssid.uSSIDLength == 0 {
                        continue;
                    }

                    let ssid_bytes = &bss.dot11Ssid.ucSSID[..bss.dot11Ssid.uSSIDLength as usize];
                    let ssid_str = String::from_utf8_lossy(ssid_bytes).to_string();
                    let ssid = Ssid(ssid_str.clone());

                    let security = backend.infer_security(bss.usCapabilityInformation);

                    let mut bssid_bytes = [0u8; 6];
                    bssid_bytes.copy_from_slice(&bss.dot11Bssid);
                    let bssid = Bssid(bssid_bytes);

                    let rssi_dbm = i8::try_from(bss.lRssi).unwrap_or(-127);
                    let (channel, band) = backend.parse_channel_info(bss.ulChCenterFrequency);

                    let radio_link = RadioLink {
                        band,
                        channel: Channel(channel as u16),
                        freq_mhz: bss.ulChCenterFrequency,
                        rssi_dbm,
                        snr_db: None,
                        last_seen: OffsetDateTime::now_utc(),
                    };

                    let access_point = AccessPoint {
                        bssid,
                        links: vec![radio_link],
                        vendor_oui: None,
                        phy_type: Some(format!("{:?}", bss.dot11BssType)),
                    };

                    if let Some(network) = network_map.get_mut(&ssid_str) {
                        network.access_points.push(access_point);
                    } else {
                        network_map.insert(
                            ssid_str,
                            Network {
                                ssid,
                                security,
                                access_points: vec![access_point],
                            },
                        );
                    }
                }

                WlanFreeMemory(bss_list as *mut c_void);
                network_map.into_values().collect()
            };

            Ok(networks)
        })
        .await
        .map_err(|e| Error::wifi(format!("Scan operation failed: {}", e)))?
    }

    async fn connect(&self, creds: &Credentials) -> Result<ConnectionInfo> {
        let backend = self.clone();
        let creds = creds.clone();

        tokio::task::spawn_blocking(move || {
            let mut backend = backend;
            let (handle, interface) = backend.ensure_initialized()?;

            let notification_mgr = NotificationManager::new(handle)?;

            let profile_xml = backend.create_profile_xml(&creds);
            let profile_xml_wide = backend.to_wide(&profile_xml);

            let mut reason_code = 0u32;
            let result = unsafe {
                WlanSetProfile(
                    handle,
                    &interface,
                    0,
                    PCWSTR(profile_xml_wide.as_ptr()),
                    None,
                    true,
                    None,
                    &mut reason_code,
                )
            };

            if result != ERROR_SUCCESS.0 {
                return Err(Error::wifi(format!(
                    "Failed to set profile: {} (reason: {})",
                    result, reason_code
                )));
            }

            std::thread::sleep(Duration::from_secs(3));

            let profile_name_wide = backend.to_wide(&creds.ssid.0);
            let connection_params = WLAN_CONNECTION_PARAMETERS {
                wlanConnectionMode: wlan_connection_mode_profile,
                strProfile: PCWSTR(profile_name_wide.as_ptr()),
                pDot11Ssid: ptr::null_mut(),
                pDesiredBssidList: ptr::null_mut(),
                dot11BssType: dot11_BSS_type_any,
                dwFlags: 0,
            };

            let result = unsafe { WlanConnect(handle, &interface, &connection_params, None) };
            if result != ERROR_SUCCESS.0 {
                return Err(Error::connection(format!("Connection failed: {}", result)));
            }

            notification_mgr
                .wait_for_event(&[WifiEvent::ConnectionComplete], Duration::from_secs(60))?;

            backend.current_connection_sync()?.ok_or_else(|| {
                Error::connection("Connected but unable to get connection info".to_string())
            })
        })
        .await
        .map_err(|e| Error::wifi(format!("Connect operation failed: {}", e)))?
    }

    async fn disconnect(&self) -> Result<()> {
        let backend = self.clone();

        tokio::task::spawn_blocking(move || {
            let mut backend = backend;
            let (handle, interface) = backend.ensure_initialized()?;

            let notification_mgr = NotificationManager::new(handle)?;

            let result = unsafe { WlanDisconnect(handle, &interface, None) };
            if result != ERROR_SUCCESS.0 {
                return Err(Error::wifi(format!("Disconnect failed: {}", result)));
            }

            notification_mgr.wait_for_event(&[WifiEvent::Disconnected], Duration::from_secs(5))?;
            Ok(())
        })
        .await
        .map_err(|e| Error::wifi(format!("Disconnect operation failed: {}", e)))?
    }

    async fn current_connection(&self) -> Result<Option<ConnectionInfo>> {
        let backend = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut backend = backend;
            backend.current_connection_sync()
        })
        .await
        .map_err(|e| Error::wifi(format!("Current connection query failed: {}", e)))?
    }

    async fn get_profiles(&self) -> Result<Vec<Credentials>> {
        let backend = self.clone();

        tokio::task::spawn_blocking(move || {
            let mut backend = backend;
            let (handle, interface) = backend.ensure_initialized()?;
            let has_admin = backend.has_admin_privileges();

            let mut profile_list: *mut WLAN_PROFILE_INFO_LIST = ptr::null_mut();
            let result = unsafe { WlanGetProfileList(handle, &interface, None, &mut profile_list) };

            if result != ERROR_SUCCESS.0 {
                return Err(Error::wifi(format!(
                    "Failed to get profile list: {}",
                    result
                )));
            }

            let profiles = unsafe {
                if profile_list.is_null() {
                    return Err(Error::wifi("Profile list is null pointer".to_string()));
                }
                let profile_count = (*profile_list).dwNumberOfItems as usize;
                let profile_entries =
                    std::slice::from_raw_parts((*profile_list).ProfileInfo.as_ptr(), profile_count);

                let mut credentials = Vec::new();

                for profile in profile_entries {
                    let profile_name = {
                        let wide_chars = &profile.strProfileName;
                        let len = wide_chars
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(wide_chars.len());
                        String::from_utf16_lossy(&wide_chars[..len])
                    };

                    let profile_name_wide = backend.to_wide(&profile_name);
                    let mut profile_xml: PWSTR = PWSTR::null();
                    let mut flags = if has_admin {
                        WLAN_PROFILE_GET_PLAINTEXT_KEY
                    } else {
                        0u32
                    };

                    let result = WlanGetProfile(
                        handle,
                        &interface,
                        PCWSTR(profile_name_wide.as_ptr()),
                        None,
                        &mut profile_xml,
                        Some(&mut flags),
                        None,
                    );

                    if result == ERROR_SUCCESS.0 && !profile_xml.is_null() {
                        let xml_string = profile_xml.to_string().unwrap_or_default();

                        let (security, passphrase) = if has_admin {
                            backend.extract_credentials_from_xml(&xml_string)
                        } else {
                            let (security, _) = backend.extract_credentials_from_xml(&xml_string);
                            (security, None)
                        };

                        credentials.push(Credentials {
                            ssid: Ssid(profile_name),
                            security,
                            passphrase,
                            created_at: OffsetDateTime::now_utc(),
                            auto_connect: false,
                            hidden: false,
                        });

                        WlanFreeMemory(profile_xml.as_ptr() as *mut c_void);
                    } else {
                        credentials.push(Credentials {
                            ssid: Ssid(profile_name),
                            security: Security::Unknown,
                            passphrase: None,
                            created_at: OffsetDateTime::now_utc(),
                            auto_connect: false,
                            hidden: false,
                        });
                    }
                }

                WlanFreeMemory(profile_list as *mut c_void);
                credentials
            };

            Ok(profiles)
        })
        .await
        .map_err(|e| Error::wifi(format!("Saved profiles query failed: {}", e)))?
    }
}
