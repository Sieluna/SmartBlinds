use core::sync::atomic::{AtomicUsize, Ordering};

use uuid::Uuid;

pub trait UuidGenerator: Send + Sync {
    /// Generate a new UUID
    fn generate(&self) -> Uuid;

    /// Generate UUID with custom data
    fn generate_with_data(&self, data: &[u8]) -> Uuid {
        let _ = data;
        self.generate()
    }
}

#[cfg(feature = "std")]
pub struct RandomUuidGenerator;

#[cfg(feature = "std")]
impl UuidGenerator for RandomUuidGenerator {
    fn generate(&self) -> Uuid {
        Uuid::new_v4()
    }
}

pub struct DeviceBasedUuidGenerator {
    device_id: [u8; 16],
    counter: AtomicUsize,
}

impl DeviceBasedUuidGenerator {
    pub fn new(device_mac: [u8; 6], device_id: u32) -> Self {
        let mut device_info = [0u8; 16];
        device_info[0..6].copy_from_slice(&device_mac);
        device_info[6..10].copy_from_slice(&device_id.to_be_bytes());

        Self {
            device_id: device_info,
            counter: AtomicUsize::new(0),
        }
    }
}

impl UuidGenerator for DeviceBasedUuidGenerator {
    fn generate(&self) -> Uuid {
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut data = self.device_id;
        data[10..].copy_from_slice(&counter.to_be_bytes()[2..]);

        Uuid::from_bytes(data)
    }

    fn generate_with_data(&self, extra_data: &[u8]) -> Uuid {
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut data = self.device_id;

        let hash = extra_data.iter().fold(counter, |acc, &b| {
            acc.wrapping_mul(31).wrapping_add(b as usize)
        });

        data[8..].copy_from_slice(&hash.to_be_bytes());
        Uuid::from_bytes(data)
    }
}
