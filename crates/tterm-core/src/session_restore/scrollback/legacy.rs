//! Bounded decoding of the gzip snapshots the Tauri version wrote.

use flate2::bufread::GzDecoder;
use std::io::Read;

const CHUNK_BYTES: usize = 8 * 1024;

/// Decompresses one gzip member of at most `max_bytes`, rejecting damage,
/// trailing data and anything larger.
///
/// The gzip trailer is read through to the end so that a matching CRC and
/// size are verified, and any bytes left after the member are refused rather
/// than ignored.
pub(super) fn decode(bytes: &[u8], max_bytes: usize) -> Option<String> {
    if bytes.len() > max_bytes {
        return None;
    }

    let mut decoder = GzDecoder::new(bytes);
    let mut content = Vec::new();
    let mut chunk = [0_u8; CHUNK_BYTES];
    loop {
        let read = decoder.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        if content.len().checked_add(read)? > max_bytes {
            return None;
        }
        content.extend_from_slice(&chunk[..read]);
    }
    if !decoder.into_inner().is_empty() {
        return None;
    }

    String::from_utf8(content).ok()
}
