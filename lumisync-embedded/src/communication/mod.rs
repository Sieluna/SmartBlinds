pub mod device;
pub mod edge;

use lumisync_api::Message;

pub trait MessageTransport {
    type Error;

    /// Send message
    fn send_message(
        &mut self,
        message: &Message,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send;

    /// Receive message
    fn receive_message(
        &mut self,
    ) -> impl core::future::Future<Output = Result<Option<Message>, Self::Error>> + Send;
}

pub use device::communicator::DeviceCommunicator;
pub use device::status::DeviceStatus;
pub use edge::analyzer::{DeviceState, EdgeAnalyzer};
pub use edge::communicator::EdgeCommunicator;
