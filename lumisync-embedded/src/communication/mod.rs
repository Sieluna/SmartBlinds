pub mod device;
pub mod edge;
pub mod transport;

use lumisync_api::Message;

#[allow(async_fn_in_trait)]
pub trait MessageTransport {
    type Error;

    /// Send message
    async fn send_message(&mut self, message: &Message) -> Result<(), Self::Error>;

    /// Receive message
    async fn receive_message(&mut self) -> Result<Option<Message>, Self::Error>;
}

pub use device::{DeviceCommunicator, DeviceStatus};
pub use edge::{EdgeAnalyzer, EdgeCommunicator};
pub use transport::*;
