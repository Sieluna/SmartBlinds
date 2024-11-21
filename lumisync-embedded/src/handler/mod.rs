use core::future::Future;
use core::pin::Pin;

use alloc::boxed::Box;
use alloc::vec::Vec;

use lumisync_api::Message;

use crate::{Error, Result};

pub mod context;
pub mod pipeline;
pub mod stages;

pub use context::*;
pub use pipeline::*;
pub use stages::*;

#[derive(Debug, Clone)]
pub enum PipelineResult<T> {
    /// Continue to the next stage
    Continue(T),
    /// Complete processing, return a response message
    Complete(Option<Message>),
    /// Skip remaining stages, complete immediately
    Skip,
    /// Processing failed
    Error(Error),
}

impl<T> PipelineResult<T> {
    pub fn is_continue(&self) -> bool {
        matches!(self, PipelineResult::Continue(_))
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, PipelineResult::Complete(_))
    }

    pub fn is_error(&self) -> bool {
        matches!(self, PipelineResult::Error(_))
    }
}

#[async_trait::async_trait]
pub trait PipelineStage<T>: Send {
    /// Stage name, for debugging
    fn name(&self) -> &'static str;

    /// Process data
    async fn process(&mut self, input: T, context: &mut dyn ProcessContext) -> PipelineResult<T>;

    /// Check if this stage should be executed
    fn should_execute(&self, _input: &T, _context: &dyn ProcessContext) -> bool {
        true
    }

    /// Get stage priority (lower number is higher priority)
    fn priority(&self) -> u8 {
        100
    }
}

/// Pipeline Executor
pub struct Pipeline<T> {
    stages: Vec<Box<dyn PipelineStage<T> + Send>>,
    name: &'static str,
}

impl<T> Pipeline<T> {
    pub fn new(name: &'static str) -> Self {
        Self {
            stages: Vec::new(),
            name,
        }
    }

    /// Add a processing stage
    pub fn add_stage<S>(mut self, stage: S) -> Self
    where
        S: PipelineStage<T> + Send + 'static,
    {
        self.stages.push(Box::new(stage));
        self
    }

    /// Sort stages by priority
    pub fn sort_by_priority(mut self) -> Self {
        self.stages.sort_by_key(|stage| stage.priority());
        self
    }

    /// Execute Pipeline
    pub async fn execute(
        &mut self,
        mut input: T,
        context: &mut dyn ProcessContext,
    ) -> Result<Option<Message>> {
        log::debug!(
            "Starting pipeline '{}' with {} stages",
            self.name,
            self.stages.len()
        );

        for (index, stage) in self.stages.iter_mut().enumerate() {
            if !stage.should_execute(&input, context) {
                log::debug!("Skipping stage '{}' (index: {})", stage.name(), index);
                continue;
            }

            log::debug!("Executing stage '{}' (index: {})", stage.name(), index);

            match stage.process(input, context).await {
                PipelineResult::Continue(next_input) => {
                    input = next_input;
                    log::debug!("Stage '{}' completed successfully", stage.name());
                }
                PipelineResult::Complete(response) => {
                    log::debug!("Pipeline completed at stage '{}'", stage.name());
                    return Ok(response);
                }
                PipelineResult::Skip => {
                    log::debug!("Stage '{}' requested skip", stage.name());
                    break;
                }
                PipelineResult::Error(error) => {
                    log::error!("Stage '{}' failed: {:?}", stage.name(), error);
                    return Err(error);
                }
            }
        }

        log::debug!("Pipeline '{}' completed successfully", self.name);
        Ok(None)
    }

    /// Get Pipeline name
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Get stage count
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }
}

#[macro_export]
macro_rules! pipeline {
    ($name:expr => $($stage:expr),* $(,)?) => {
        Pipeline::new($name)
            $(.add_stage($stage))*
            .sort_by_priority()
    };
}

#[macro_export]
macro_rules! stage {
    ($name:expr, |$input:ident, $ctx:ident| $body:expr) => {
        struct AnonymousStage;

        #[async_trait::async_trait]
        impl<T> PipelineStage<T> for AnonymousStage {
            fn name(&self) -> &'static str {
                $name
            }

            async fn process(
                &mut self,
                $input: T,
                $ctx: &mut dyn ProcessContext,
            ) -> PipelineResult<T> {
                $body
            }
        }

        AnonymousStage
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestStage {
        name: &'static str,
        should_fail: bool,
    }

    impl TestStage {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                should_fail: false,
            }
        }

        fn with_failure(mut self) -> Self {
            self.should_fail = true;
            self
        }
    }

    #[async_trait::async_trait]
    impl PipelineStage<i32> for TestStage {
        fn name(&self) -> &'static str {
            self.name
        }

        async fn process(
            &mut self,
            input: i32,
            _context: &mut dyn ProcessContext,
        ) -> PipelineResult<i32> {
            if self.should_fail {
                PipelineResult::Error(Error::InvalidState)
            } else {
                PipelineResult::Continue(input + 1)
            }
        }
    }

    #[tokio::test]
    async fn test_pipeline_execution() {
        use crate::handler::context::MockContext;

        let mut pipeline = Pipeline::new("test")
            .add_stage(TestStage::new("stage1"))
            .add_stage(TestStage::new("stage2"))
            .add_stage(TestStage::new("stage3"));

        let mut context = MockContext::new();
        let result = pipeline.execute(0, &mut context).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pipeline_failure() {
        use crate::handler::context::MockContext;

        let mut pipeline = Pipeline::new("test_fail")
            .add_stage(TestStage::new("stage1"))
            .add_stage(TestStage::new("stage2").with_failure())
            .add_stage(TestStage::new("stage3"));

        let mut context = MockContext::new();
        let result = pipeline.execute(0, &mut context).await;

        assert!(result.is_err());
    }

    #[test]
    fn test_pipeline_macros() {
        let _pipeline = pipeline!(
            "test_macro" =>
            TestStage::new("macro_stage1"),
            TestStage::new("macro_stage2"),
        );

        assert_eq!(_pipeline.stage_count(), 2);
        assert_eq!(_pipeline.name(), "test_macro");
    }
}
