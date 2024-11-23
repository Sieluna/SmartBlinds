#[derive(Debug, Clone)]
pub struct NetworkStatus {
    pub total_nodes: usize,
    pub synced_nodes: usize,
    pub failed_nodes: usize,
    pub average_accuracy_ms: f32,
}

impl NetworkStatus {
    pub fn new() -> Self {
        Self {
            total_nodes: 0,
            synced_nodes: 0,
            failed_nodes: 0,
            average_accuracy_ms: 0.0,
        }
    }

    pub fn sync_ratio(&self) -> f32 {
        if self.total_nodes == 0 {
            0.0
        } else {
            self.synced_nodes as f32 / self.total_nodes as f32
        }
    }

    pub fn failure_ratio(&self) -> f32 {
        if self.total_nodes == 0 {
            0.0
        } else {
            self.failed_nodes as f32 / self.total_nodes as f32
        }
    }
}

impl Default for NetworkStatus {
    fn default() -> Self {
        Self::new()
    }
}
