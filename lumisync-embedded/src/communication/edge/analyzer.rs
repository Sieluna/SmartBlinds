use alloc::collections::BTreeMap;

use lumisync_api::Id;

pub struct EdgeAnalyzer {
    /// Device status cache
    device_states: BTreeMap<Id, DeviceState>,
}

#[derive(Debug, Clone)]
pub struct DeviceState {
    pub position: u8,
    pub battery_level: u8,
    pub last_update: Option<u64>, // Simplified timestamp
}

impl EdgeAnalyzer {
    pub fn new() -> Self {
        Self {
            device_states: BTreeMap::new(),
        }
    }

    /// Update device status
    pub fn update_device_state(&mut self, device_id: Id, position: u8, battery: u8) {
        self.device_states.insert(
            device_id,
            DeviceState {
                position,
                battery_level: battery,
                last_update: Some(0), // Simplified implementation
            },
        );
    }

    /// Get device status
    pub fn get_device_state(&self, device_id: Id) -> Option<&DeviceState> {
        self.device_states.get(&device_id)
    }

    /// Analyze if device adjustment is needed
    pub fn analyze_adjustment_needed(&self, device_id: Id) -> Option<u8> {
        if let Some(state) = self.device_states.get(&device_id) {
            // Simplified analysis logic: if battery is below 20%, suggest closing window to save power
            if state.battery_level < 20 {
                return Some(0); // Suggest closing
            }
        }
        None
    }
}

impl Default for EdgeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_analyzer_low_battery() {
        let mut analyzer = EdgeAnalyzer::new();
        analyzer.update_device_state(1, 50, 15); // Low battery

        let adjustment = analyzer.analyze_adjustment_needed(1);
        assert_eq!(adjustment, Some(0)); // Suggest closing
    }

    #[test]
    fn test_edge_analyzer_normal_battery() {
        let mut analyzer = EdgeAnalyzer::new();
        analyzer.update_device_state(1, 50, 80); // Normal battery

        let adjustment = analyzer.analyze_adjustment_needed(1);
        assert_eq!(adjustment, None); // No adjustment needed
    }
}
