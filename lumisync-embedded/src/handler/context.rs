use core::any::{Any, TypeId};
use core::sync::atomic::{AtomicU64, Ordering};

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use lumisync_api::{Message, NodeId};
use time::OffsetDateTime;

use crate::Error;
use crate::protocol::MessageBuilder;
use crate::time::TimeSync;

pub trait ProcessContext: Send + Sync {
    /// Get current message
    fn current_message(&self) -> Option<&Message>;

    /// Set current message
    fn set_current_message(&mut self, message: Message);

    /// Get source node
    fn source_node(&self) -> Option<&NodeId>;

    /// Get target node
    fn target_node(&self) -> Option<&NodeId>;

    /// Get time synchronizer
    fn time_sync(&self) -> &TimeSync;

    /// Get message builder
    fn message_builder(&self) -> &MessageBuilder;

    /// Get or create extension data
    fn extensions(&self) -> &Extensions;

    /// Get mutable extension data
    fn extensions_mut(&mut self) -> &mut Extensions;

    /// Set error state
    fn set_error(&mut self, error: Error);

    /// Get error state
    fn get_error(&self) -> Option<&Error>;

    /// Add response message
    fn add_response(&mut self, message: Message);

    /// Get all response messages
    fn take_responses(&mut self) -> Vec<Message>;

    /// Get processing statistics
    fn get_stats(&self) -> &ProcessStats;

    /// Update processing statistics
    fn get_stats_mut(&mut self) -> &mut ProcessStats;
}

#[derive(Default)]
pub struct Extensions {
    data: BTreeMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Extensions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert data
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) {
        self.data.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Get data
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.data
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref())
    }

    /// Get mutable data
    pub fn get_mut<T: Send + Sync + 'static>(&mut self) -> Option<&mut T> {
        self.data
            .get_mut(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_mut())
    }

    /// Remove data
    pub fn remove<T: Send + Sync + 'static>(&mut self) -> Option<T> {
        self.data
            .remove(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast().ok().map(|boxed| *boxed))
    }

    /// Check if contains type
    pub fn contains<T: Send + Sync + 'static>(&self) -> bool {
        self.data.contains_key(&TypeId::of::<T>())
    }
}

#[derive(Debug, Default)]
pub struct ProcessStats {
    pub messages_processed: AtomicU64,
    pub messages_sent: AtomicU64,
    pub errors_count: AtomicU64,
    pub last_activity: Option<OffsetDateTime>,
    pub start_time: Option<OffsetDateTime>,
}

impl ProcessStats {
    pub fn new() -> Self {
        Self {
            start_time: Some(OffsetDateTime::now_utc()),
            ..Default::default()
        }
    }

    pub fn increment_processed(&self) {
        self.messages_processed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_sent(&self) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_errors(&self) {
        self.errors_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_processed(&self) -> u64 {
        self.messages_processed.load(Ordering::Relaxed)
    }

    pub fn get_sent(&self) -> u64 {
        self.messages_sent.load(Ordering::Relaxed)
    }

    pub fn get_errors(&self) -> u64 {
        self.errors_count.load(Ordering::Relaxed)
    }
}

pub struct BaseContext {
    current_message: Option<Message>,
    time_sync: TimeSync,
    message_builder: MessageBuilder,
    extensions: Extensions,
    error: Option<Error>,
    responses: Vec<Message>,
    stats: ProcessStats,
}

impl BaseContext {
    pub fn new(time_sync: TimeSync, message_builder: MessageBuilder) -> Self {
        Self {
            current_message: None,
            time_sync,
            message_builder,
            extensions: Extensions::new(),
            error: None,
            responses: Vec::new(),
            stats: ProcessStats::new(),
        }
    }

    /// Update activity time
    pub fn update_activity(&mut self) {
        self.stats.last_activity = Some(OffsetDateTime::now_utc());
    }
}

impl ProcessContext for BaseContext {
    fn current_message(&self) -> Option<&Message> {
        self.current_message.as_ref()
    }

    fn set_current_message(&mut self, message: Message) {
        self.current_message = Some(message);
        self.update_activity();
    }

    fn source_node(&self) -> Option<&NodeId> {
        self.current_message.as_ref().map(|m| &m.header.source)
    }

    fn target_node(&self) -> Option<&NodeId> {
        self.current_message.as_ref().map(|m| &m.header.target)
    }

    fn time_sync(&self) -> &TimeSync {
        &self.time_sync
    }

    fn message_builder(&self) -> &MessageBuilder {
        &self.message_builder
    }

    fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }

    fn set_error(&mut self, error: Error) {
        self.error = Some(error);
        self.stats.increment_errors();
    }

    fn get_error(&self) -> Option<&Error> {
        self.error.as_ref()
    }

    fn add_response(&mut self, message: Message) {
        self.responses.push(message);
        self.stats.increment_sent();
    }

    fn take_responses(&mut self) -> Vec<Message> {
        core::mem::take(&mut self.responses)
    }

    fn get_stats(&self) -> &ProcessStats {
        &self.stats
    }

    fn get_stats_mut(&mut self) -> &mut ProcessStats {
        &mut self.stats
    }
}

#[cfg(test)]
pub struct MockContext {
    base: BaseContext,
}

#[cfg(test)]
impl MockContext {
    pub fn new() -> Self {
        use alloc::sync::Arc;

        use crate::protocol::uuid_generator::DeviceBasedUuidGenerator;

        let time_sync = TimeSync::new();
        let uuid_gen = Arc::new(DeviceBasedUuidGenerator::new([0; 6], 0));
        let message_builder = MessageBuilder::new(lumisync_api::Protocol::default(), uuid_gen);

        Self {
            base: BaseContext::new(time_sync, message_builder),
        }
    }
}

#[cfg(test)]
impl ProcessContext for MockContext {
    fn current_message(&self) -> Option<&Message> {
        self.base.current_message()
    }

    fn set_current_message(&mut self, message: Message) {
        self.base.set_current_message(message);
    }

    fn source_node(&self) -> Option<&NodeId> {
        self.base.source_node()
    }

    fn target_node(&self) -> Option<&NodeId> {
        self.base.target_node()
    }

    fn time_sync(&self) -> &TimeSync {
        self.base.time_sync()
    }

    fn message_builder(&self) -> &MessageBuilder {
        self.base.message_builder()
    }

    fn extensions(&self) -> &Extensions {
        self.base.extensions()
    }

    fn extensions_mut(&mut self) -> &mut Extensions {
        self.base.extensions_mut()
    }

    fn set_error(&mut self, error: Error) {
        self.base.set_error(error);
    }

    fn get_error(&self) -> Option<&Error> {
        self.base.get_error()
    }

    fn add_response(&mut self, message: Message) {
        self.base.add_response(message);
    }

    fn take_responses(&mut self) -> Vec<Message> {
        self.base.take_responses()
    }

    fn get_stats(&self) -> &ProcessStats {
        self.base.get_stats()
    }

    fn get_stats_mut(&mut self) -> &mut ProcessStats {
        self.base.get_stats_mut()
    }
}
