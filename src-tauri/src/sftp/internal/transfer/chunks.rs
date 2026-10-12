//! The completed-chunk bitmap and the sizes of chunks and wire requests.

use super::{chunk_len_for, failed, TransferError, DEFAULT_IO_STEP, MIN_CHUNK_SIZE};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use russh_sftp::extensions::LimitsExtension;

pub(super) fn bitmap_len(chunk_count: usize) -> usize {
    chunk_count.div_ceil(8)
}

pub(super) fn bitmap_get(bits: &[u8], index: usize) -> bool {
    bits.get(index / 8)
        .map(|byte| byte & (1 << (index % 8)) != 0)
        .unwrap_or(false)
}

pub(super) fn bitmap_set(bits: &mut [u8], index: usize) {
    if let Some(byte) = bits.get_mut(index / 8) {
        *byte |= 1 << (index % 8);
    }
}

pub(super) fn bitmap_count(bits: &[u8], chunk_count: usize) -> usize {
    (0..chunk_count)
        .filter(|index| bitmap_get(bits, *index))
        .count()
}

pub(super) fn completed_bytes(
    bits: &[u8],
    chunk_size: u64,
    total_size: u64,
    chunk_count: usize,
) -> u64 {
    (0..chunk_count)
        .filter(|index| bitmap_get(bits, *index))
        .map(|index| chunk_len_for(index, chunk_size, total_size))
        .sum()
}

pub(super) fn encode_bitmap(bits: &[u8]) -> String {
    BASE64.encode(bits)
}

pub(super) fn decode_bitmap(encoded: &str, chunk_count: usize) -> Result<Vec<u8>, TransferError> {
    let decoded = BASE64
        .decode(encoded)
        .map_err(|err| failed(format!("Corrupt transfer checkpoint bitmap: {err}")))?;
    let mut bits = vec![0u8; bitmap_len(chunk_count)];
    for (index, byte) in decoded.iter().enumerate() {
        if index < bits.len() {
            bits[index] = *byte;
        }
    }
    Ok(bits)
}

pub(super) fn step_from_limits(limits: Option<&LimitsExtension>) -> u64 {
    let mut step = 32 * 1024u64;
    if let Some(limits) = limits {
        if limits.max_write_len > 0 {
            step = step.min(limits.max_write_len);
        }
        if limits.max_read_len > 0 {
            step = step.min(limits.max_read_len);
        }
        if limits.max_packet_len > 0 {
            step = step.min(limits.max_packet_len.saturating_sub(1024));
        }
    }
    step.max(1)
}

/// Largest single write request a lane will issue for upload chunks.
pub(super) fn io_write_step(limits: Option<&LimitsExtension>) -> u64 {
    let mut step = limits
        .and_then(|limits| (limits.max_write_len > 0).then_some(limits.max_write_len))
        .unwrap_or(DEFAULT_IO_STEP);
    if let Some(limits) = limits {
        if limits.max_packet_len > 0 {
            step = step.min(limits.max_packet_len.saturating_sub(1024));
        }
    }
    step.max(1)
}

/// Largest single read request a lane will issue for download chunks.
pub(super) fn io_read_step(limits: Option<&LimitsExtension>) -> u64 {
    let mut step = limits
        .and_then(|limits| (limits.max_read_len > 0).then_some(limits.max_read_len))
        .unwrap_or(DEFAULT_IO_STEP);
    if let Some(limits) = limits {
        if limits.max_packet_len > 0 {
            step = step.min(limits.max_packet_len.saturating_sub(1024));
        }
    }
    step.max(1)
}

/// Chunk size after small-file shrinking.
///
/// A lane pipelines *chunks*, and each chunk sends its steps serially. When
/// the server does not advertise `limits@openssh.com` the logical chunk
/// (4 MiB) is far larger than one write step (256 KiB), so a file smaller
/// than a handful of chunks would run almost serially. Shrink the chunk to
/// `max(one write step, total / window)` in that case so the sliding window
/// becomes request-level pipelining. Never grows the chunk, never shrinks
/// below one write step, and leaves large transfers and empty transfers
/// alone: the chunk stays the logical 4 MiB resume unit even when the wire
/// step is smaller (the chunk is deliberately never clamped by the server's
/// read/write limits — `io_write_step`/`io_read_step` bound each request).
pub(super) fn effective_chunk_size(
    negotiated: u64,
    step: u64,
    total_size: u64,
    window: usize,
) -> u64 {
    if total_size == 0 {
        return negotiated;
    }
    let target = (total_size / window.max(1) as u64).max(1);
    negotiated.min(step.max(target)).max(MIN_CHUNK_SIZE)
}

/// Highest offset the part file must cover for the completed bitmap to be
/// believable: the end of the last completed chunk (the final chunk ends at
/// `total_size`).
pub(super) fn required_part_size(
    bits: &[u8],
    chunk_size: u64,
    total_size: u64,
    chunk_count: usize,
) -> u64 {
    (0..chunk_count)
        .filter(|index| bitmap_get(bits, *index))
        .map(|index| ((index as u64 + 1) * chunk_size).min(total_size))
        .max()
        .unwrap_or(0)
}
