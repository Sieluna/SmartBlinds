use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    Command, DeviceStatus, Id, RegionSettingData, SensorData, WindowData, WindowSettingData,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum NodeId {
    /// Cloud node
    Cloud,
    /// Edge node (1 byte ID)
    Edge(u8),
    /// Device node (6 byte MAC)
    Device([u8; 6]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    /// Normal operation message
    Regular,
    /// High priority message that requires immediate attention
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message metadata
    pub header: MessageHeader,
    /// Message content
    pub payload: MessagePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    /// Unique message identifier
    pub id: Uuid,
    /// Message creation timestamp
    pub timestamp: OffsetDateTime,
    /// Message priority level
    pub priority: Priority,
    /// Message source identifier
    pub source: NodeId,
    /// Message destination identifier
    pub target: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    /// Command from cloud to edge devices
    CloudCommand(CloudCommand),
    /// Status report from edge to cloud
    EdgeReport(EdgeReport),
    /// Command from edge to device
    EdgeCommand(EdgeCommand),
    /// Report from device to edge
    DeviceReport(DeviceReport),
    /// Time synchronization messages
    TimeSync(TimeSyncPayload),
    /// Success confirmation response
    Acknowledge(AckPayload),
    /// Error response with details
    Error(ErrorPayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeSyncPayload {
    /// Time synchronization request
    Request {
        /// Request sequence number
        sequence: u32,
        /// Send time
        send_time: OffsetDateTime,
        /// Precision requirement (milliseconds)
        precision_ms: u16,
    },
    /// Time synchronization response
    Response {
        /// Corresponding request sequence number
        request_sequence: u32,
        /// Request receive time
        request_receive_time: OffsetDateTime,
        /// Response send time
        response_send_time: OffsetDateTime,
        /// Estimated network delay (milliseconds)
        estimated_delay_ms: u32,
        /// Time accuracy (milliseconds)
        accuracy_ms: u16,
    },
    /// Time offset broadcast (mainly used for edge nodes to broadcast to devices)
    Broadcast {
        /// Current timestamp
        timestamp: OffsetDateTime,
        /// Time offset (milliseconds)
        offset_ms: i64,
        /// Accuracy assessment (milliseconds)
        accuracy_ms: u16,
    },
    /// Time synchronization status query
    StatusQuery,
    /// Time synchronization status response
    StatusResponse {
        /// Whether it is synchronized
        is_synced: bool,
        /// Current time offset (milliseconds)
        current_offset_ms: i64,
        /// Last synchronization time
        last_sync_time: OffsetDateTime,
        /// Synchronization accuracy (milliseconds)
        accuracy_ms: u16,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CloudCommand {
    /// Update region environmental settings
    ConfigureRegion {
        /// New region settings
        plan: Vec<RegionSettingData>,
    },
    /// Update window control settings
    ConfigureWindow {
        /// Target window identifier
        device_id: Id,
        /// New window settings
        plan: Vec<WindowSettingData>,
    },
    /// Send commands to multiple devices
    ControlDevices {
        /// Device commands map
        commands: BTreeMap<Id, Command>,
    },
    /// Send optimization suggestions
    SendAnalyse {
        /// Suggested window positions
        windows: BTreeMap<Id, WindowData>,
        /// Analysis explanation
        reason: String,
        /// Confidence score
        confidence: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeReport {
    /// Device status update
    DeviceStatus {
        /// Device status list
        devices: BTreeMap<Id, DeviceStatus>,
    },
    /// Edge system health metrics
    HealthReport {
        /// CPU usage percentage
        cpu_usage: f32,
        /// Memory usage percentage
        memory_usage: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActuatorCommand {
    /// Set window position
    SetWindowPosition(u8),
    /// Request current motor status
    RequestStatus,
    /// Emergency stop
    EmergencyStop,
    /// Calibrate actuator
    Calibrate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeCommand {
    /// Command to control a specific actuator
    Actuator {
        /// Sequence number for command tracking
        sequence: u16,
        /// Command to execute
        command: ActuatorCommand,
    },
    /// Request for health status report
    RequestHealthStatus,
    /// Request for sensor data
    RequestSensorData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceReport {
    /// Status update from an actuator
    Status {
        /// Window position data
        window_data: WindowData,
        /// Battery level percentage
        battery_level: u8,
        /// Error code (0 = no error)
        error_code: u8,
        /// Relative timestamp from device boot (milliseconds)
        relative_timestamp: u64,
    },
    /// Sensor data update
    SensorData {
        /// Sensor readings
        sensor_data: SensorData,
        /// Relative timestamp from device boot (milliseconds)
        relative_timestamp: u64,
    },
    /// Device health metrics
    HealthStatus {
        /// CPU usage percentage
        cpu_usage: f32,
        /// Memory usage percentage
        memory_usage: f32,
        /// Battery level percentage
        battery_level: u8,
        /// Signal strength (RSSI)
        signal_strength: i8,
        /// Relative timestamp from device boot (milliseconds)
        relative_timestamp: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckPayload {
    /// Reference to original message
    pub original_msg_id: Uuid,
    /// Operation status
    pub status: String,
    /// Additional status information
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    /// Reference to failed message
    pub original_msg_id: Option<Uuid>,
    /// Error type
    pub code: ErrorCode,
    /// Error description
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCode {
    /// Invalid input parameters
    InvalidRequest,
    /// Device not connected
    DeviceOffline,
    /// Operation not allowed
    PermissionDenied,
    /// Resource limit exceeded
    OverLimit,
    /// Internal processing error
    InternalError,
    /// Device hardware error
    HardwareFailure,
    /// Communication error
    NetworkError,
    /// Critical battery level
    BatteryLow,
    /// Operation time exceeded
    Timeout,
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use crate::transport::{Protocol, deserialize, serialize};

    use super::*;

    fn create_device_status_message() -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Device([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]),
                target: NodeId::Edge(1),
            },
            payload: MessagePayload::DeviceReport(DeviceReport::Status {
                window_data: WindowData {
                    target_position: 75,
                },
                battery_level: 85,
                error_code: 0,
                relative_timestamp: 1000,
            }),
        }
    }

    fn create_edge_command_message() -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Emergency,
                source: NodeId::Edge(1),
                target: NodeId::Device([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]),
            },
            payload: MessagePayload::EdgeCommand(EdgeCommand::Actuator {
                sequence: 1234,
                command: ActuatorCommand::SetWindowPosition(50),
            }),
        }
    }

    fn create_cloud_command_message() -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Cloud,
                target: NodeId::Edge(1),
            },
            payload: MessagePayload::CloudCommand(CloudCommand::ConfigureWindow {
                device_id: 1,
                plan: vec![WindowSettingData {
                    position_range: (20, 80),
                    auto_adjust: true,
                }],
            }),
        }
    }

    fn create_ack_message() -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Device([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]),
                target: NodeId::Edge(1),
            },
            payload: MessagePayload::Acknowledge(AckPayload {
                original_msg_id: Uuid::nil(),
                status: String::from("OK"),
                details: Some(String::from("Position set to 50%")),
            }),
        }
    }

    fn create_error_message() -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Emergency,
                source: NodeId::Device([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]),
                target: NodeId::Edge(1),
            },
            payload: MessagePayload::Error(ErrorPayload {
                original_msg_id: Some(Uuid::nil()),
                code: ErrorCode::HardwareFailure,
                message: String::from("Motor driver failure detected"),
            }),
        }
    }

    fn create_sensor_data_message() -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::nil(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Device([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]),
                target: NodeId::Edge(1),
            },
            payload: MessagePayload::DeviceReport(DeviceReport::SensorData {
                sensor_data: SensorData {
                    temperature: 23.5,
                    illuminance: 1500,
                    humidity: 65.2,
                },
                relative_timestamp: 1000,
            }),
        }
    }

    #[test]
    fn test_message_sizes() {
        let samples = vec![
            ("Device Status", create_device_status_message()),
            ("Edge Control", create_edge_command_message()),
            ("Cloud Config", create_cloud_command_message()),
            ("ACK", create_ack_message()),
            ("Error", create_error_message()),
            ("Sensor Data", create_sensor_data_message()),
        ];

        for (name, msg) in samples {
            let serialized = serialize(Protocol::Postcard, &msg).expect("Serialization failed");
            let size = serialized.len();
            // Assert BLE MTU limit (<=244 bytes after which fragmentation needed)
            assert!(
                size <= 244,
                "{} message size {} exceeds BLE MTU limit",
                name,
                size
            );
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        let protocols = vec![Protocol::Postcard, Protocol::Json];

        let samples: Vec<Message> = vec![
            create_device_status_message(),
            create_edge_command_message(),
            create_cloud_command_message(),
            create_ack_message(),
            create_error_message(),
            create_sensor_data_message(),
        ];

        for protocol in &protocols {
            for original in &samples {
                let bytes = serialize(*protocol, original).expect("serialize failed");
                let decoded: Message = deserialize(*protocol, &bytes).expect("deserialize failed");
                assert_eq!(original.header.id, decoded.header.id);
            }
        }
    }
}
