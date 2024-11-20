use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    Command, DeviceStatus, Id, RegionSettingData, SensorData, WindowData, WindowSettingData,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum NodeId {
    /// Cloud node
    Cloud,
    /// Edge node (1 byte ID)
    Edge(u8),
    /// Device node (6 byte MAC)
    Device([u8; 6]),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
    /// Success confirmation response
    Acknowledge(AckPayload),
    /// Error response with details
    Error(ErrorPayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CloudCommand {
    /// Update region environmental settings
    ConfigureRegion {
        /// Target region identifier
        region_id: Id,
        /// New region settings
        plan: Vec<RegionSettingData>,
    },
    /// Update window control settings
    ConfigureWindow {
        /// Target window identifier
        window_id: Id,
        /// New window settings
        plan: Vec<WindowSettingData>,
    },
    /// Send commands to multiple devices
    ControlDevices {
        /// Target region identifier
        region_id: Id,
        /// Device commands map
        commands: BTreeMap<Id, Command>,
    },
    /// Send optimization suggestions
    SendAnalyse {
        /// Target region identifier
        region_id: Id,
        /// Suggested window positions
        windows: Vec<WindowData>,
        /// Analysis explanation
        reason: String,
        /// Confidence score
        confidence: f32,
    },
    /// Time synchronization command
    TimeSync {
        /// Current cloud server UTC time
        cloud_time: OffsetDateTime,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeReport {
    /// Device status update
    DeviceStatus {
        /// Source region identifier
        region_id: Id,
        /// Device status list
        devices: Vec<DeviceStatus>,
    },
    /// Edge system health metrics
    HealthReport {
        /// CPU usage percentage
        cpu_usage: f32,
        /// Memory usage percentage
        memory_usage: f32,
    },
    /// Time synchronization request
    RequestTimeSync {
        /// Edge's current local time
        local_time: OffsetDateTime,
        /// Time offset from last sync (milliseconds)
        current_offset_ms: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActuatorCommand {
    /// Set window position
    SetWindowPosition(u8),
    /// Request current status
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
        /// Actuator/device identifier
        actuator_id: Id,
        /// Sequence number for command tracking
        sequence: u16,
        /// Command to execute
        command: ActuatorCommand,
    },
    /// Request for health status report
    RequestHealthStatus {
        /// Actuator/device identifier
        actuator_id: Id,
    },
    /// Request for sensor data
    RequestSensorData {
        /// Actuator/device identifier
        actuator_id: Id,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceReport {
    /// Status update from an actuator
    Status {
        /// Actuator/device identifier
        actuator_id: Id,
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
        /// Actuator/device identifier
        actuator_id: Id,
        /// Sensor readings
        sensor_data: SensorData,
        /// Relative timestamp from device boot (milliseconds)
        relative_timestamp: u64,
    },
    /// Device health metrics
    HealthStatus {
        /// Actuator/device identifier
        device_id: Id,
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
                actuator_id: 42,
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
                actuator_id: 42,
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
                window_id: 1,
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
                actuator_id: 10,
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
