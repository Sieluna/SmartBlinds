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
#[derive(Clone)]
pub struct RandomUuidGenerator;

#[cfg(feature = "std")]
impl UuidGenerator for RandomUuidGenerator {
    fn generate(&self) -> Uuid {
        Uuid::new_v4()
    }
}

pub struct DeviceBasedUuidGenerator {
    device_mac: [u8; 6],
    counter: AtomicUsize,
}

impl DeviceBasedUuidGenerator {
    pub fn new(device_mac: [u8; 6]) -> Self {
        Self {
            device_mac,
            counter: AtomicUsize::new(0),
        }
    }
}

impl UuidGenerator for DeviceBasedUuidGenerator {
    fn generate(&self) -> Uuid {
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut bytes = [0u8; 16];
        bytes[0..6].copy_from_slice(&self.device_mac);
        bytes[6..14].copy_from_slice(&(counter as u64).to_be_bytes());
        Uuid::from_bytes(bytes)
    }

    fn generate_with_data(&self, extra_data: &[u8]) -> Uuid {
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut bytes = [0u8; 16];
        bytes[0..6].copy_from_slice(&self.device_mac);

        let hash = extra_data.iter().fold(counter as u64, |acc, &b| {
            acc.wrapping_mul(31).wrapping_add(b as u64)
        });

        bytes[6..14].copy_from_slice(&hash.to_be_bytes());
        Uuid::from_bytes(bytes)
    }
}

impl Clone for DeviceBasedUuidGenerator {
    fn clone(&self) -> Self {
        Self {
            device_mac: self.device_mac,
            counter: AtomicUsize::new(self.counter.load(Ordering::SeqCst)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_unique_uuids() {
        let mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let generator = DeviceBasedUuidGenerator::new(mac);

        let uuid1 = generator.generate();
        let uuid2 = generator.generate();

        assert_ne!(uuid1, uuid2, "UUIDs should be unique");
    }

    #[test]
    fn test_generate_with_data_same_data() {
        let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let generator = DeviceBasedUuidGenerator::new(mac);
        let data = b"test_data";

        let uuid1 = generator.generate_with_data(data);
        let uuid2 = generator.generate_with_data(data);

        assert_ne!(
            uuid1, uuid2,
            "UUIDs with same data should be different due to counter"
        );
    }

    #[test]
    fn test_generate_with_data_different_data() {
        let mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let generator = DeviceBasedUuidGenerator::new(mac);

        let uuid1 = generator.generate_with_data(b"data1");
        let uuid2 = generator.generate_with_data(b"data2");

        assert_ne!(
            uuid1, uuid2,
            "UUIDs with different data should be different"
        );
    }

    #[test]
    fn test_mac_address_embedding() {
        let mac = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];
        let generator = DeviceBasedUuidGenerator::new(mac);
        let uuid = generator.generate();

        let bytes = uuid.as_bytes();
        assert_eq!(&bytes[0..6], &mac, "First 6 bytes should match MAC address");
    }

    #[test]
    fn test_counter_increases() {
        let mac = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let generator = DeviceBasedUuidGenerator::new(mac);

        let uuid1 = generator.generate();
        let counter_part1 = &uuid1.as_bytes()[6..14];

        let uuid2 = generator.generate();
        let counter_part2 = &uuid2.as_bytes()[6..14];

        let count1 = u64::from_be_bytes(counter_part1.try_into().unwrap());
        let count2 = u64::from_be_bytes(counter_part2.try_into().unwrap());

        assert_eq!(count1 + 1, count2, "Counter should increment by 1");
    }
}
