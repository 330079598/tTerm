use tokio::io::AsyncWriteExt;

struct SequenceEnd {
    index: usize,
    terminator_len: usize,
}

/// Process SSH output for terminal queries and write replies back through SSH channel.
///
/// Terminal applications like vim send escape sequence queries to discover terminal capabilities.
/// If we don't respond, they will hang waiting forever. This function intercepts those queries,
/// sends appropriate responses back through the SSH channel, and returns the display output for UI.
///
/// The display output is raw bytes: SSH data packets split at arbitrary byte
/// boundaries, so decoding to text here would corrupt multi-byte characters
/// that straddle a packet boundary. UTF-8-safe chunking is the output
/// batcher's job (`crate::terminal::io_batcher`).
///
/// Supported queries:
/// - Device Attributes (DA): terminal identification
/// - Cursor Position Report (CPR): current cursor location
/// - Operating Status Report (OSR): terminal ready status
/// - XTGETTCAP: terminal capability queries (e.g., color support)
/// - OSC color queries: foreground/background colors
pub async fn process_ssh_output_for_ui<W>(
    data: &[u8],
    pending: &mut Vec<u8>,
    writer: &mut W,
) -> std::io::Result<Vec<u8>>
where
    W: AsyncWriteExt + Unpin,
{
    let mut input = Vec::with_capacity(pending.len() + data.len());
    if !pending.is_empty() {
        input.extend_from_slice(pending);
        pending.clear();
    }
    input.extend_from_slice(data);

    let mut output = Vec::new();
    let mut i = 0;

    while i < input.len() {
        if input[i] != 0x1b {
            output.push(input[i]);
            i += 1;
            continue;
        }

        let rest = &input[i..];
        let mut matched = false;

        // Secondary Device Attributes (DA): ESC[>c
        // Response format: ESC[>{version};{firmware};{hardware}c
        // We respond as VT400 with custom firmware 276
        if rest.len() >= 4 && rest.starts_with(b"\x1b[>") {
            // Limit search to reasonable sequence length (max 32 bytes)
            let search_range = &rest[0..rest.len().min(32)];
            if let Some(end) = search_range.iter().position(|&b| b == b'c') {
                let params = &rest[3..end];
                let valid_da = params.iter().all(|b| b.is_ascii_digit() || *b == b';');
                if valid_da {
                    writer.write_all(b"\x1b[>0;276;0c").await?;
                    writer.flush().await?;
                    i += end + 1;
                    matched = true;
                }
            }
        }

        // Cursor Position Report (CPR): ESC[6n
        // Response format: ESC[{row};{col}R
        // We report cursor at home position (1,1)
        if !matched && rest.starts_with(b"\x1b[6n") {
            writer.write_all(b"\x1b[1;1R").await?;
            writer.flush().await?;
            i += 4;
            matched = true;
        }

        // DEC CPR: ESC[?6n (DECCIR format)
        // Response format: ESC[?{row};{col}R
        if !matched && rest.starts_with(b"\x1b[?6n") {
            writer.write_all(b"\x1b[?1;1R").await?;
            writer.flush().await?;
            i += 5;
            matched = true;
        }

        // Operating Status Report (OSR): ESC[5n
        // Response format: ESC{status}n (0 = ready, 3 = failure)
        if !matched && rest.starts_with(b"\x1b[5n") {
            writer.write_all(b"\x1b[0n").await?;
            writer.flush().await?;
            i += 4;
            matched = true;
        }

        // Primary Device Attributes (DA): ESC[c or ESC[?c
        // Response format: ESC[?{attributes}c
        // Attributes: 64=VT420, 1=132cols, 2=printer, 4=select, 6=softkeys,
        //              9=window, 15=tech, 21=horizontal scrolling, 22=color
        if !matched && rest.len() >= 3 && rest.starts_with(b"\x1b[") {
            let start = if rest[2] == b'?' { 3 } else { 2 };
            if rest.len() > start && rest[start] == b'c' {
                writer.write_all(b"\x1b[?64;1;2;4;6;9;15;21;22c").await?;
                writer.flush().await?;
                i += start + 1;
                matched = true;
            }
        }

        // DCS XTGETTCAP: ESC P + q <hex> ESC \
        // Terminal capability query (xterm extension)
        // Response: ESC P {0/1} + r {hex} ESC \ (0=unknown, 1=supported)
        if !matched && rest.starts_with(b"\x1bP+q") {
            if let Some(end) = find_dcs_end(rest) {
                let hex_query = &rest[4..end.index];

                // "Co" = 436f/436F in hex (terminal colors capability)
                if hex_query == b"436f" || hex_query == b"436F" {
                    // Respond with 256 colors: 1+r436f3d323536 = "Co=256"
                    writer.write_all(b"\x1bP1+r436f3d323536\x1b\\").await?;
                } else {
                    // Unknown capability - respond with failure
                    writer.write_all(b"\x1bP0+r").await?;
                    writer.write_all(hex_query).await?;
                    writer.write_all(b"\x1b\\").await?;
                }
                writer.flush().await?;
                i += end.index + end.terminator_len;
                matched = true;
            }
        }

        // OSC color queries: ESC]10;? (foreground) or ESC]11;? (background)
        // Operating System Command to query terminal colors
        if !matched && rest.starts_with(b"\x1b]") {
            if let Some(end) = find_osc_end(rest) {
                let body = &rest[2..end.index];
                if body.starts_with(b"10;?") {
                    // Foreground color: respond with light gray
                    writer
                        .write_all(b"\x1b]10;rgb:aaaa/aaaa/aaaa\x1b\\")
                        .await?;
                    writer.flush().await?;
                    i += end.index + end.terminator_len;
                    matched = true;
                } else if body.starts_with(b"11;?") {
                    // Background color: respond with dark gray
                    writer
                        .write_all(b"\x1b]11;rgb:1111/1111/1111\x1b\\")
                        .await?;
                    writer.flush().await?;
                    i += end.index + end.terminator_len;
                    matched = true;
                }
            }
        }

        if !matched && is_incomplete_query(rest) {
            pending.extend_from_slice(rest);
            break;
        }

        if !matched {
            output.push(input[i]);
            i += 1;
        }
    }

    Ok(output)
}

fn is_incomplete_query(rest: &[u8]) -> bool {
    if rest.starts_with(b"\x1bP+q") {
        return find_dcs_end(rest).is_none();
    }

    if rest.starts_with(b"\x1b]") {
        return find_osc_end(rest).is_none();
    }

    if rest.starts_with(b"\x1b[>") {
        let search_range = &rest[0..rest.len().min(32)];
        if search_range.iter().all(|b| *b != b'c') {
            return rest.len() < 32;
        }
    }

    let fixed_queries: [&[u8]; 5] = [b"\x1b[6n", b"\x1b[?6n", b"\x1b[5n", b"\x1b[c", b"\x1b[?c"];
    if fixed_queries.iter().any(|query| query.starts_with(rest)) {
        return true;
    }

    rest == b"\x1b"
        || rest == b"\x1b["
        || rest == b"\x1b[?"
        || rest == b"\x1bP"
        || rest == b"\x1bP+"
}

/// Find the end of a DCS (Device Control String) sequence.
/// DCS starts with ESC P and ends with ST (0x9c) or ESC \
fn find_dcs_end(data: &[u8]) -> Option<SequenceEnd> {
    for i in 2..data.len() {
        if data[i] == 0x9c {
            return Some(SequenceEnd {
                index: i,
                terminator_len: 1,
            });
        }
        if data[i] == 0x1b && i + 1 < data.len() && data[i + 1] == b'\\' {
            return Some(SequenceEnd {
                index: i,
                terminator_len: 2,
            });
        }
    }
    None
}

/// Find the end of an OSC (Operating System Command) sequence.
/// OSC starts with ESC ] and ends with BEL (0x07) or ST (ESC \
fn find_osc_end(data: &[u8]) -> Option<SequenceEnd> {
    for i in 2..data.len() {
        if data[i] == 0x07 {
            return Some(SequenceEnd {
                index: i,
                terminator_len: 1,
            });
        }
        if data[i] == 0x1b && i + 1 < data.len() && data[i + 1] == b'\\' {
            return Some(SequenceEnd {
                index: i,
                terminator_len: 2,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds `data` through the processor in fixed-size slices, as SSH data
    /// packets would, and returns the concatenated display output plus every
    /// byte written back through the channel.
    async fn process_in_chunks(
        data: &[u8],
        chunk_size: usize,
    ) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut pending = Vec::new();
        let mut replies = Vec::new();
        let mut output = Vec::new();
        for chunk in data.chunks(chunk_size.max(1)) {
            output.extend_from_slice(
                &process_ssh_output_for_ui(chunk, &mut pending, &mut replies)
                    .await
                    .expect("process chunk"),
            );
        }
        (output, replies, pending)
    }

    #[tokio::test]
    async fn multibyte_chars_split_across_packets_survive() {
        // 7 slices through the middle of both 3-byte and 4-byte sequences.
        let text = "中文测试 😀 tail\r\n";
        let (output, replies, pending) = process_in_chunks(text.as_bytes(), 7).await;
        assert!(replies.is_empty());
        assert!(pending.is_empty());
        assert_eq!(output, text.as_bytes());
    }

    #[tokio::test]
    async fn single_char_split_at_packet_boundary_survives() {
        // "中" is E4 B8 AD; a packet ending after E4 B8 must defer those bytes.
        let text = "中";
        let (output, _, _) = process_in_chunks(text.as_bytes(), 2).await;
        assert_eq!(output, text.as_bytes());
    }

    #[tokio::test]
    async fn device_attributes_query_is_answered_and_consumed() {
        let (output, replies, _) = process_in_chunks(b"before\x1b[cafter", 8).await;
        assert_eq!(
            replies,
            b"\x1b[?64;1;2;4;6;9;15;21;22c".to_vec(),
            "primary DA must be answered"
        );
        assert_eq!(output, b"beforeafter".to_vec());
    }

    #[tokio::test]
    async fn cursor_position_report_is_answered_and_consumed() {
        let (output, replies, _) = process_in_chunks(b"\x1b[6n", 8).await;
        assert_eq!(replies, b"\x1b[1;1R".to_vec());
        assert!(output.is_empty());
    }

    #[tokio::test]
    async fn incomplete_sequence_tail_is_deferred_to_pending() {
        // A CSI cut off by a packet boundary must wait for the next packet.
        let (first_output, replies, pending) =
            process_in_chunks(b"hi\x1b[", 16).await;
        assert_eq!(first_output, b"hi".to_vec());
        assert!(replies.is_empty());
        assert_eq!(pending, b"\x1b[".to_vec());

        let mut pending = pending;
        let mut replies = Vec::new();
        let second = process_ssh_output_for_ui(b"31m", &mut pending, &mut replies)
            .await
            .expect("process completion chunk");
        assert!(replies.is_empty());
        assert!(pending.is_empty());
        assert_eq!(second, b"\x1b[31m".to_vec());
    }
}
