/// CRC-32 implementation using IEEE 802.3 polynomial: 0xEDB88320
pub struct Crc32 {
    table: [u32; 256],
}

impl Crc32 {
    const POLYNOMIAL: u32 = 0xEDB88320;

    /// Creates a new CRC-32 instance with pre-computed lookup table
    pub fn new() -> Self {
        let mut table = [0u32; 256];

        for (i, slot) in table.iter_mut().enumerate() {
            let mut crc = i as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ Self::POLYNOMIAL;
                } else {
                    crc >>= 1;
                }
            }
            *slot = crc;
        }

        Self { table }
    }

    /// Computes CRC-32 checksum for the given data
    pub fn checksum(&self, data: &[u8]) -> u32 {
        let mut crc = 0xFFFFFFFF;

        for &byte in data {
            let index = ((crc ^ byte as u32) & 0xFF) as usize;
            crc = (crc >> 8) ^ self.table[index];
        }

        crc ^ 0xFFFFFFFF
    }

    /// Updates CRC value with additional data (for streaming computation)
    pub fn update(&self, mut crc: u32, data: &[u8]) -> u32 {
        for &byte in data {
            let index = ((crc ^ byte as u32) & 0xFF) as usize;
            crc = (crc >> 8) ^ self.table[index];
        }
        crc
    }

    /// Returns initial CRC value for streaming computation
    pub fn init() -> u32 {
        0xFFFFFFFF
    }

    /// Finalizes CRC computation
    pub fn finalize(crc: u32) -> u32 {
        crc ^ 0xFFFFFFFF
    }
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

/// Global CRC-32 instance with compile-time computed lookup table
static CRC32: Crc32 = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ Crc32::POLYNOMIAL;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    Crc32 { table }
};

/// Computes CRC-32 checksum using the global instance
pub fn crc32(data: &[u8]) -> u32 {
    CRC32.checksum(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_crc32_known_values() {
        // Test against known CRC-32 values
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"a"), 0xe8b7be43);
        assert_eq!(crc32(b"abc"), 0x352441c2);
        assert_eq!(crc32(b"message digest"), 0x20159d7f);
        assert_eq!(crc32(b"abcdefghijklmnopqrstuvwxyz"), 0x4c2750bd);
        assert_eq!(
            crc32(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"),
            0x1fc2e6d2
        );
    }

    #[test]
    fn test_crc32_incremental() {
        let data = b"Hello, World!";
        let full_crc = crc32(data);

        let crc32_inst = Crc32::new();
        let mut crc = Crc32::init();
        crc = crc32_inst.update(crc, &data[..5]);
        crc = crc32_inst.update(crc, &data[5..]);
        let incremental_crc = Crc32::finalize(crc);

        assert_eq!(full_crc, incremental_crc);
    }

    #[test]
    fn test_crc32_edge_cases() {
        assert_eq!(crc32(&[]), 0);
        assert_eq!(crc32(&[0]), 0xd202ef8d);
        assert_eq!(crc32(&[255]), 0xff000000);

        let repeated = vec![0x42; 1000];
        let crc1 = crc32(&repeated);
        let crc2 = crc32(&repeated);
        assert_eq!(crc1, crc2);
    }

    #[test]
    fn test_crc32_consistency() {
        let data = b"test data for consistency check";

        let crc1 = crc32(data);
        let crc32_inst = Crc32::new();
        let crc2 = crc32_inst.checksum(data);

        assert_eq!(crc1, crc2);
    }
}
