use alloc::vec::Vec;

use lumisync_api::NodeId;

use super::stages::{
    AuthenticationStage, ErrorHandlingStage, LogLevel, LoggingStage, ValidationStage,
};
use super::{Pipeline, PipelineStage};

pub struct PipelineBuilder<T> {
    pipeline: Pipeline<T>,
}

impl<T> PipelineBuilder<T> {
    pub fn new(name: &'static str) -> Self {
        Self {
            pipeline: Pipeline::new(name),
        }
    }

    /// Add basic validation and authentication stages (Message type only)
    pub fn with_basic_security(self, allowed_sources: Vec<NodeId>) -> Self
    where
        T: 'static,
        ValidationStage: PipelineStage<T>,
        AuthenticationStage: PipelineStage<T>,
    {
        self.add_stage(ValidationStage::new())
            .add_stage(AuthenticationStage::new().allow_sources(allowed_sources))
    }

    /// Add logging (Message type only)
    pub fn with_logging(self, level: LogLevel, include_payload: bool) -> Self
    where
        T: 'static,
        LoggingStage: PipelineStage<T>,
    {
        let mut logging_stage = LoggingStage::new(level);
        if include_payload {
            logging_stage = logging_stage.include_payload();
        }
        self.add_stage(logging_stage)
    }

    /// Add error handling (Message type only)
    pub fn with_error_handling(self, auto_recovery: bool) -> Self
    where
        T: 'static,
        ErrorHandlingStage: PipelineStage<T>,
    {
        self.add_stage(
            ErrorHandlingStage::new()
                .with_auto_recovery(auto_recovery)
                .with_logging(true),
        )
    }

    /// Add stage
    pub fn add_stage<S>(mut self, stage: S) -> Self
    where
        S: PipelineStage<T> + Send + 'static,
    {
        self.pipeline = self.pipeline.add_stage(stage);
        self
    }

    /// Build Pipeline
    pub fn build(self) -> Pipeline<T> {
        self.pipeline.sort_by_priority()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PerformanceProfile {
    /// Minimal resource usage
    Minimal,
    /// Standard configuration
    Standard,
    /// High performance configuration
    HighPerformance,
}

pub struct PipelineUtils;

impl PipelineUtils {
    /// Check Pipeline health status
    pub fn check_pipeline_health<T>(pipeline: &Pipeline<T>) -> bool {
        pipeline.stage_count() > 0
    }

    /// Get recommended performance configuration
    pub fn get_recommended_performance_profile(available_memory_kb: u32) -> PerformanceProfile {
        if available_memory_kb < 64 {
            PerformanceProfile::Minimal
        } else if available_memory_kb < 256 {
            PerformanceProfile::Standard
        } else {
            PerformanceProfile::HighPerformance
        }
    }
}

#[cfg(test)]
mod tests {
    use lumisync_api::Message;

    use super::*;

    #[test]
    fn test_pipeline_builder() {
        let pipeline: Pipeline<Message> = PipelineBuilder::new("test")
            .with_logging(LogLevel::Debug, false)
            .with_basic_security(alloc::vec![NodeId::Cloud])
            .with_error_handling(true)
            .build();

        assert_eq!(pipeline.name(), "test");
        assert!(pipeline.stage_count() > 0);
    }

    #[test]
    fn test_performance_profile() {
        // Test low resource configuration
        let low_mem_profile = PipelineUtils::get_recommended_performance_profile(32);
        assert!(matches!(low_mem_profile, PerformanceProfile::Minimal));

        // Test high resource configuration
        let high_mem_profile = PipelineUtils::get_recommended_performance_profile(512);
        assert!(matches!(
            high_mem_profile,
            PerformanceProfile::HighPerformance
        ));
    }

    #[test]
    fn test_pipeline_health() {
        let pipeline: Pipeline<Message> = PipelineBuilder::new("health_test").build();
        assert!(!PipelineUtils::check_pipeline_health(&pipeline)); // Empty pipeline is unhealthy

        let pipeline_with_stages: Pipeline<Message> = PipelineBuilder::new("health_test")
            .with_logging(LogLevel::Info, false)
            .build();
        assert!(PipelineUtils::check_pipeline_health(&pipeline_with_stages)); // Pipeline with stages is healthy
    }
}
