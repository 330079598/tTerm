//! CRC16 (CCITT/XMODEM variant, used by ZHEX and ZBIN headers) and CRC32
//! (used by ZBIN32 headers and data subpackets once CRC-32 is negotiated).
//!
//! Both algorithms and their transmission byte order were verified against
//! real `lrzsz` traffic, not just the written spec: e.g. the captured ZRINIT
//! header `01 00 00 00 23` carries CRC16 `0xbe50`, and the captured ZFILE
//! header `04 00 00 00 01` carries CRC32 `0x44a5614b` transmitted
//! little-endian (`4b 61 a5 44` on the wire) — both reproduced exactly by
//! the functions below (see the unit tests).

const CRC16_POLY: u16 = 0x1021;

/// CRC-16/XMODEM: poly 0x1021, init 0, MSB-first, no input/output reflection.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ CRC16_POLY
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Standard IEEE CRC-32 (the same algorithm `crc32fast` implements
/// elsewhere in this codebase), reused here rather than hand-rolled.
pub fn crc32(data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc16_matches_the_xmodem_check_value() {
        // Canonical CRC-16/XMODEM check value for the ASCII string "123456789".
        assert_eq!(crc16(b"123456789"), 0x31c3);
    }

    #[test]
    fn crc16_matches_real_lrzsz_headers() {
        // type + 4 data bytes, captured verbatim from a real sz<->rz session.
        assert_eq!(crc16(&[0x00, 0x00, 0x00, 0x00, 0x00]), 0x0000); // ZRQINIT
        assert_eq!(crc16(&[0x01, 0x00, 0x00, 0x00, 0x23]), 0xbe50); // ZRINIT
        assert_eq!(crc16(&[0x09, 0x00, 0x00, 0x00, 0x00]), 0xa87c); // ZRPOS
        assert_eq!(crc16(&[0x08, 0x00, 0x00, 0x00, 0x00]), 0x022d); // ZFIN
    }

    #[test]
    fn crc32_matches_real_lrzsz_header() {
        // ZFILE header type+data, captured verbatim; wire bytes were the
        // little-endian encoding of this value (`4b 61 a5 44`).
        assert_eq!(crc32(&[0x04, 0x00, 0x00, 0x00, 0x01]), 0x44a5614b);
    }
}
