use crate::scheduler::types::Task;
use async_trait::async_trait;

/// A source of external tasks that can be polled for new work.
#[async_trait]
pub trait TaskSource: Send + Sync {
    /// Fetch all currently available tasks from this source.
    async fn fetch_tasks(&self) -> anyhow::Result<Vec<Task>>;

    /// Returns a static string identifying this source type.
    #[allow(dead_code)]
    fn source_type(&self) -> &'static str;
}
