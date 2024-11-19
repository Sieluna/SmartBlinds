use std::net::SocketAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepperCommand {
    /// Move by specified number of steps
    Move(i32),
    /// Set maximum speed in steps per second
    SetSpeed(f32),
    /// Set acceleration in steps per second squared
    SetAcceleration(f32),
    /// Return to home position
    Home,
    /// Emergency stop
    Stop,
    /// Query current status
    Status,
    /// Connection health check
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepperResponse {
    /// Command executed successfully
    Ok,
    /// Command execution failed
    Error(String),
    /// Current motor status
    Status {
        position: i32,
        target: i32,
        speed: f32,
        running: bool,
    },
    /// Health check response
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepperCommandResult {
    /// The executed command
    pub command: StepperCommand,
    /// Response from the motor controller
    pub response: StepperResponse,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Whether the command was executed successfully
    pub success: bool,
}

#[derive(Debug, Clone)]
pub struct StepperControllerConfig {
    /// Timeout for establishing TCP connection
    pub connection_timeout: Duration,
    /// Timeout for command execution
    pub command_timeout: Duration,
}

impl Default for StepperControllerConfig {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(10),
        }
    }
}

pub struct StepperController {
    config: StepperControllerConfig,
}

impl StepperController {
    pub fn new() -> Self {
        Self {
            config: StepperControllerConfig::default(),
        }
    }

    /// Create a new stepper controller with custom configuration
    pub fn with_config(config: StepperControllerConfig) -> Self {
        Self { config }
    }

    /// Execute a stepper motor command
    pub async fn execute_command(
        &self,
        endpoint: &str,
        command: StepperCommand,
    ) -> Result<StepperCommandResult> {
        let start_time = std::time::Instant::now();

        let result = self.send_command(endpoint, &command).await;
        let duration_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(response) => Ok(StepperCommandResult {
                command: command.clone(),
                response,
                duration_ms,
                success: true,
            }),
            Err(e) => Ok(StepperCommandResult {
                command: command.clone(),
                response: StepperResponse::Error(e.to_string()),
                duration_ms,
                success: false,
            }),
        }
    }

    /// Validate command parameters before execution
    pub fn validate_command(&self, command: &StepperCommand) -> Result<()> {
        match command {
            StepperCommand::Move(steps) => {
                if steps.abs() > 10000 {
                    return Err(Error::stepper(
                        "Move steps must be between -10000 and 10000",
                    ));
                }
            }
            StepperCommand::SetSpeed(speed) => {
                if *speed <= 0.0 || *speed > 2000.0 {
                    return Err(Error::stepper(
                        "Speed must be between 0.1 and 2000.0 steps/sec",
                    ));
                }
            }
            StepperCommand::SetAcceleration(accel) => {
                if *accel <= 0.0 || *accel > 1000.0 {
                    return Err(Error::stepper(
                        "Acceleration must be between 0.1 and 1000.0 steps/sec²",
                    ));
                }
            }
            _ => {} // Other commands don't require validation
        }
        Ok(())
    }

    /// Send command to stepper motor
    async fn send_command(
        &self,
        endpoint: &str,
        command: &StepperCommand,
    ) -> Result<StepperResponse> {
        // Parse endpoint
        let addr = self.parse_endpoint(endpoint)?;

        // Connect to device
        let mut stream = timeout(self.config.connection_timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| Error::connection_to("Connection timeout", endpoint))?
            .map_err(|e| Error::connection_to(format!("Failed to connect: {}", e), endpoint))?;

        // Serialize command
        let command_json = serde_json::to_string(command)?;

        // Send command
        timeout(
            self.config.command_timeout,
            stream.write_all(command_json.as_bytes()),
        )
        .await
        .map_err(|_| Error::stepper("Command send timeout"))?
        .map_err(|e| Error::stepper(format!("Failed to send command: {}", e)))?;

        // Read response
        let mut buffer = [0u8; 1024];
        let bytes_read = timeout(self.config.command_timeout, stream.read(&mut buffer))
            .await
            .map_err(|_| Error::stepper("Response timeout"))?
            .map_err(|e| Error::stepper(format!("Failed to read response: {}", e)))?;

        // Parse response
        let response_str = std::str::from_utf8(&buffer[..bytes_read])
            .map_err(|e| Error::serialization(format!("Invalid UTF-8 in response: {}", e)))?;

        let response: StepperResponse = serde_json::from_str(response_str)?;

        Ok(response)
    }

    /// Parse endpoint string to socket address
    fn parse_endpoint(&self, endpoint: &str) -> Result<SocketAddr> {
        // Handle different endpoint formats
        let addr_str = if endpoint.contains("://") {
            // Extract address from URL (e.g. "http://192.168.1.100:8082")
            let url_parts: Vec<&str> = endpoint.split("://").collect();
            if url_parts.len() != 2 {
                return Err(Error::stepper(format!(
                    "Invalid endpoint format: {}",
                    endpoint
                )));
            }
            url_parts[1]
        } else {
            endpoint
        };

        // Add default port if not specified
        let final_addr = if addr_str.contains(':') {
            addr_str.to_string()
        } else {
            format!("{}:8082", addr_str)
        };

        final_addr
            .parse::<SocketAddr>()
            .map_err(|e| Error::stepper(format!("Invalid endpoint address {}: {}", final_addr, e)))
    }
}

impl Default for StepperController {
    fn default() -> Self {
        Self::new()
    }
}
