use alloc::collections::VecDeque;
use alloc::vec::Vec;

use lumisync_api::message::{
    AckPayload, EdgeCommand, Message, MessageHeader, MessagePayload, NodeId, Priority,
};
use lumisync_api::uuid::{DeviceBasedUuidGenerator, UuidGenerator};

use crate::device::command_executor::{CommandExecution, DeviceCommandExecutor};
use crate::error::{Error, Result};

pub struct DeviceMessageHandler {
    device_mac: [u8; 6],
    uuid_generator: DeviceBasedUuidGenerator,
    command_executor: DeviceCommandExecutor,
    connected_edge_id: Option<u8>,
    message_queue: VecDeque<Message>,
    response_queue: VecDeque<Message>,
    max_queue_size: usize,
}

impl DeviceMessageHandler {
    pub fn new(device_mac: [u8; 6], uuid_generator: DeviceBasedUuidGenerator) -> Self {
        Self {
            device_mac,
            uuid_generator,
            command_executor: DeviceCommandExecutor::new(device_mac),
            connected_edge_id: None,
            message_queue: VecDeque::new(),
            response_queue: VecDeque::new(),
            max_queue_size: 50,
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        self.command_executor.initialize_hardware().await?;
        Ok(())
    }

    pub fn connect_to_edge(&mut self, edge_id: u8) {
        self.connected_edge_id = Some(edge_id);
        self.message_queue.clear();
        self.response_queue.clear();
    }

    pub fn disconnect_from_edge(&mut self) {
        self.connected_edge_id = None;
        self.message_queue.clear();
        self.response_queue.clear();
    }

    pub fn receive_message(&mut self, message: Message) -> Result<()> {
        if let Some(edge_id) = self.connected_edge_id {
            if let NodeId::Edge(msg_edge_id) = message.header.source {
                if msg_edge_id != edge_id {
                    return Err(Error::InvalidCommand);
                }
            } else {
                return Err(Error::InvalidCommand);
            }
        } else {
            return Err(Error::NotConnected);
        }

        if self.message_queue.len() >= self.max_queue_size {
            return Err(Error::NetworkError);
        }

        self.message_queue.push_back(message);
        Ok(())
    }

    pub async fn process_messages(&mut self) -> Result<usize> {
        let mut processed_count = 0;

        while let Some(message) = self.message_queue.pop_front() {
            match self.process_single_message(message).await {
                Ok(response) => {
                    if let Some(resp) = response {
                        self.response_queue.push_back(resp);
                    }
                    processed_count += 1;
                }
                Err(error) => {
                    log::error!("Failed to process message: {:?}", error);
                }
            }
        }

        Ok(processed_count)
    }

    async fn process_single_message(&mut self, message: Message) -> Result<Option<Message>> {
        match &message.payload {
            MessagePayload::EdgeCommand(edge_command) => {
                self.handle_edge_command(&message, edge_command).await
            }
            _ => Ok(None),
        }
    }

    async fn handle_edge_command(
        &mut self,
        original_message: &Message,
        command: &EdgeCommand,
    ) -> Result<Option<Message>> {
        let execution_result = self.command_executor.execute_edge_command(command).await?;
        let response = self.create_command_response(original_message, &execution_result)?;
        Ok(Some(response))
    }

    fn create_command_response(
        &mut self,
        original_message: &Message,
        execution: &CommandExecution,
    ) -> Result<Message> {
        let response_id = self.uuid_generator.generate();

        let response_header = MessageHeader {
            id: response_id,
            source: NodeId::Device(self.device_mac),
            target: original_message.header.source,
            timestamp: time::OffsetDateTime::now_utc(),
            priority: Priority::Regular,
        };

        let response_payload =
            if execution.status == crate::device::command_executor::ExecutionStatus::Completed {
                MessagePayload::Acknowledge(AckPayload {
                    original_msg_id: original_message.header.id,
                    status: "Success".into(),
                    details: execution.result.as_ref().map(|r| format!("{:?}", r)),
                })
            } else {
                MessagePayload::Error(lumisync_api::message::ErrorPayload {
                    original_msg_id: Some(original_message.header.id),
                    code: lumisync_api::message::ErrorCode::InternalError,
                    message: execution
                        .error
                        .clone()
                        .unwrap_or_else(|| "Unknown error".into()),
                })
            };

        Ok(Message {
            header: response_header,
            payload: response_payload,
        })
    }

    pub fn get_pending_responses(&mut self) -> Vec<Message> {
        self.response_queue.drain(..).collect()
    }

    pub fn has_pending_responses(&self) -> bool {
        !self.response_queue.is_empty()
    }

    pub fn get_queue_status(&self) -> MessageQueueStatus {
        MessageQueueStatus {
            pending_messages: self.message_queue.len(),
            pending_responses: self.response_queue.len(),
            max_queue_size: self.max_queue_size,
            connected_edge: self.connected_edge_id,
        }
    }

    pub fn get_executor_stats(&self) -> crate::device::command_executor::ExecutorStats {
        self.command_executor.get_executor_stats()
    }

    pub fn get_command_history(&self) -> &alloc::collections::BTreeMap<u32, CommandExecution> {
        self.command_executor.get_command_history()
    }

    pub async fn cancel_command(&mut self, sequence: u32) -> Result<()> {
        self.command_executor.cancel_command(sequence).await
    }

    pub fn clear_queues(&mut self) {
        self.message_queue.clear();
        self.response_queue.clear();
    }

    pub fn set_max_queue_size(&mut self, max_size: usize) {
        self.max_queue_size = max_size;

        while self.message_queue.len() > max_size {
            self.message_queue.pop_front();
        }

        while self.response_queue.len() > max_size {
            self.response_queue.pop_front();
        }
    }
}

#[derive(Debug, Clone)]
pub struct MessageQueueStatus {
    pub pending_messages: usize,
    pub pending_responses: usize,
    pub max_queue_size: usize,
    pub connected_edge: Option<u8>,
}

impl MessageQueueStatus {
    pub fn is_queue_nearly_full(&self) -> bool {
        let usage_percentage =
            (self.pending_messages + self.pending_responses) as f64 / self.max_queue_size as f64;
        usage_percentage > 0.8
    }

    pub fn is_connected(&self) -> bool {
        self.connected_edge.is_some()
    }
}
