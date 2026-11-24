use tokio::io::AsyncWriteExt;

struct SequenceEnd {
    index: usize,
    terminator_len: usize,
}

/// Who answers the standard Device Status Report queries.
///
/// `CSI 5 n` (status) and `CSI 6 n` (cursor position) are the standard,
/// non-private members of the ECMA-48 DSR family. The DEC-private
/// `CSI ? 6 n` is not one of them and keeps its own fixed reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandardDsrReply {
    /// The SSH layer writes the fixed reply the Tauri frontend expects.
    ///
    /// That is `CSI 0 n` for status and `CSI 1 ; 1 R` for the cursor.
    SshLayer,
    /// The query is forwarded verbatim to the caller's terminal model.
    ///
    /// The GPUI session parses the output in its own model, which answers from
    /// the real terminal state through the same ordered path as user input.
    /// Forwarding each query once is the whole contract; the SSH layer writes
    /// no reply of its own.
    Model,
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
/// - Device Status Reports (DSR): status (`CSI 5 n`) and cursor position
///   (`CSI 6 n`), answered here or forwarded to the model depending on
///   [`StandardDsrReply`]
/// - XTGETTCAP: terminal capability queries (e.g., color support)
///
/// OSC 10/11 color queries pass through: xterm.js answers them with the tab's
/// own theme, which programs such as Codex blend their panel colors from.
pub async fn process_ssh_output_for_ui<W>(
    data: &[u8],
    pending: &mut Vec<u8>,
    writer: &mut W,
    dsr_reply: StandardDsrReply,
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

        // Standard DSR cursor position report: ESC[6n
        // Response format: ESC[{row};{col}R
        // Without a model we report the cursor at home (1,1). A frontend with
        // a model gets the query forwarded and answers it from its own grid.
        if !matched && rest.starts_with(b"\x1b[6n") {
            route_standard_dsr(b"\x1b[6n", b"\x1b[1;1R", dsr_reply, writer, &mut output).await?;
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

        // Standard DSR status report: ESC[5n
        // Response format: ESC{status}n (0 = ready, 3 = failure)
        // The model reports ready as well; forwarding keeps one responder.
        if !matched && rest.starts_with(b"\x1b[5n") {
            route_standard_dsr(b"\x1b[5n", b"\x1b[0n", dsr_reply, writer, &mut output).await?;
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
        if !matched
            && rest.starts_with(b"\x1bP+q")
            && let Some(end) = find_dcs_end(rest)
        {
            let hex_query = &rest[4..end.index];

            // "Co" = 436f/436F in hex (terminal colors capability)
            if hex_query == b"436f" || hex_query == b"436F" {
                // Respond with 256 colors: 1+r436f3d323536 = "Co=256"
                writer.write_all(b"\x1bP1+r436f3d323536\x1b\\").await?;
            } else if hex_query.eq_ignore_ascii_case(b"4d73") {
                writer.write_all(XTGETTCAP_MS_REPLY).await?;
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

/// Writes the SSH layer's fixed answer to a standard Device Status Report, or
/// appends the query to the display output so the caller's model answers it.
async fn route_standard_dsr<W>(
    query: &[u8],
    legacy_reply: &[u8],
    reply: StandardDsrReply,
    writer: &mut W,
    output: &mut Vec<u8>,
) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    match reply {
        StandardDsrReply::SshLayer => {
            writer.write_all(legacy_reply).await?;
            writer.flush().await?;
        }
        StandardDsrReply::Model => output.extend_from_slice(query),
    }
    Ok(())
}

/// Longest tail held back waiting for the rest of a query. The queries
/// answered here are a few dozen bytes at most; anything still unterminated
/// past this is ordinary output that xterm.js parses across chunks itself.
/// Without the cap, an unterminated `ESC P+q` (truncated output, a binary
/// `cat`) would hold back everything after it until a stray ST showed up.
const MAX_DEFERRED_QUERY_BYTES: usize = 64;

/// XTGETTCAP answer for "Ms", the terminfo string that sets the clipboard
/// with OSC 52 (`\E]52;%p1%s;%p2%s\007`, hex-encoded). Neovim asks for it
/// before it copies that way. It is answered even when the setting that lets
/// programs copy is off, since the frontend then drops what arrives.
const XTGETTCAP_MS_REPLY: &[u8] = b"\x1bP1+r4d73=5c455d35323b25703125733b25703225735c303037\x1b\\";

fn is_incomplete_query(rest: &[u8]) -> bool {
    if rest.len() >= MAX_DEFERRED_QUERY_BYTES {
        return false;
    }

    if rest.starts_with(b"\x1bP+q") {
        return find_dcs_end(rest).is_none();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds `data` through the processor in fixed-size slices, as SSH data
    /// packets would, and returns the concatenated display output plus every
    /// byte written back through the channel.
    async fn process_in_chunks(data: &[u8], chunk_size: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        process_in_chunks_with_dsr(data, chunk_size, StandardDsrReply::SshLayer).await
    }

    async fn process_in_chunks_with_dsr(
        data: &[u8],
        chunk_size: usize,
        dsr_reply: StandardDsrReply,
    ) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut pending = Vec::new();
        let mut replies = Vec::new();
        let mut output = Vec::new();
        for chunk in data.chunks(chunk_size.max(1)) {
            output.extend_from_slice(
                &process_ssh_output_for_ui(chunk, &mut pending, &mut replies, dsr_reply)
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
    async fn standard_dsr_queries_are_answered_and_consumed() {
        // The Tauri sink keeps this path: the SSH layer answers both standard
        // reports and none of the bytes reach the frontend.
        let (output, replies, _) = process_in_chunks(b"\x1b[5n\x1b[6n", 8).await;
        assert_eq!(replies, b"\x1b[0n\x1b[1;1R".to_vec());
        assert!(output.is_empty());
    }

    #[tokio::test]
    async fn cursor_position_report_is_forwarded_to_the_model() {
        let (output, replies, pending) =
            process_in_chunks_with_dsr(b"\x1b[6n", 8, StandardDsrReply::Model).await;
        assert!(
            replies.is_empty(),
            "the SSH layer must not answer for the model"
        );
        assert!(pending.is_empty());
        assert_eq!(output, b"\x1b[6n".to_vec());
    }

    #[tokio::test]
    async fn status_report_is_forwarded_to_the_model() {
        let (output, replies, pending) =
            process_in_chunks_with_dsr(b"\x1b[5n", 8, StandardDsrReply::Model).await;
        assert!(
            replies.is_empty(),
            "the model must be the only responder to a status report"
        );
        assert!(pending.is_empty());
        assert_eq!(output, b"\x1b[5n".to_vec());
    }

    #[tokio::test]
    async fn cursor_position_report_split_across_packets_is_forwarded_once() {
        // `ESC [` arrives alone, then `6n`; the model must see one whole query.
        let (output, replies, pending) =
            process_in_chunks_with_dsr(b"\x1b[6n", 3, StandardDsrReply::Model).await;
        assert!(replies.is_empty());
        assert!(pending.is_empty());
        assert_eq!(output, b"\x1b[6n".to_vec());
    }

    #[tokio::test]
    async fn status_report_split_across_packets_is_forwarded_once() {
        let (output, replies, pending) =
            process_in_chunks_with_dsr(b"\x1b[5n", 3, StandardDsrReply::Model).await;
        assert!(replies.is_empty());
        assert!(pending.is_empty());
        assert_eq!(output, b"\x1b[5n".to_vec());
    }

    #[tokio::test]
    async fn standard_dsr_queries_coalesced_in_one_packet_are_forwarded_in_order() {
        let (output, replies, pending) =
            process_in_chunks_with_dsr(b"\x1b[5n\x1b[6n", 64, StandardDsrReply::Model).await;
        assert!(replies.is_empty());
        assert!(pending.is_empty());
        assert_eq!(output, b"\x1b[5n\x1b[6n".to_vec());
    }

    #[tokio::test]
    async fn reverse_ordered_standard_dsr_queries_keep_their_order() {
        let (output, replies, pending) =
            process_in_chunks_with_dsr(b"\x1b[6n\x1b[5n", 64, StandardDsrReply::Model).await;
        assert!(replies.is_empty());
        assert!(pending.is_empty());
        assert_eq!(output, b"\x1b[6n\x1b[5n".to_vec());
    }

    #[tokio::test]
    async fn adjacent_cursor_position_reports_are_forwarded_separately() {
        let (output, replies, pending) =
            process_in_chunks_with_dsr(b"\x1b[6n\x1b[6n", 5, StandardDsrReply::Model).await;
        assert!(replies.is_empty());
        assert!(pending.is_empty());
        assert_eq!(output, b"\x1b[6n\x1b[6n".to_vec());
    }

    #[tokio::test]
    async fn forwarding_standard_dsr_leaves_the_other_queries_with_the_ssh_layer() {
        // DA, the DEC-private CPR and XTGETTCAP keep their transport answers;
        // only the two standard DSRs follow the model.
        let data = b"\x1b[c\x1b[?6n\x1b[5n\x1bP+q436f\x1b\\\x1b[6n";
        let (output, replies, pending) =
            process_in_chunks_with_dsr(data, 4, StandardDsrReply::Model).await;
        assert!(pending.is_empty());
        assert_eq!(output, b"\x1b[5n\x1b[6n".to_vec());
        assert_eq!(
            replies,
            b"\x1b[?64;1;2;4;6;9;15;21;22c\x1b[?1;1R\x1bP1+r436f3d323536\x1b\\".to_vec()
        );
    }

    #[tokio::test]
    async fn incomplete_sequence_tail_is_deferred_to_pending() {
        // A CSI cut off by a packet boundary must wait for the next packet.
        let (first_output, replies, pending) = process_in_chunks(b"hi\x1b[", 16).await;
        assert_eq!(first_output, b"hi".to_vec());
        assert!(replies.is_empty());
        assert_eq!(pending, b"\x1b[".to_vec());

        let mut pending = pending;
        let mut replies = Vec::new();
        let second = process_ssh_output_for_ui(
            b"31m",
            &mut pending,
            &mut replies,
            StandardDsrReply::SshLayer,
        )
        .await
        .expect("process completion chunk");
        assert!(replies.is_empty());
        assert!(pending.is_empty());
        assert_eq!(second, b"\x1b[31m".to_vec());
    }

    #[tokio::test]
    async fn color_queries_reach_the_terminal() {
        // xterm.js answers them with the tab's theme; a fixed reply here would
        // tell every program the background is dark gray.
        let data = b"a\x1b]10;?\x1b\\\x1b]11;?\x07b";
        let (output, replies, pending) = process_in_chunks(data, 3).await;
        assert!(replies.is_empty());
        assert!(pending.is_empty());
        assert_eq!(output, data.to_vec());
    }

    #[tokio::test]
    async fn unterminated_osc_does_not_hold_back_later_output() {
        // A truncated title sequence followed by ordinary output, none of it
        // containing BEL or ST: the output must reach the terminal now.
        let data = b"\x1b]0;half a title\r\nls output\r\n$ ";
        let (output, replies, pending) = process_in_chunks(data, 8).await;
        assert!(replies.is_empty());
        assert!(pending.is_empty());
        assert_eq!(output, data.to_vec());
    }

    #[tokio::test]
    async fn large_osc_streams_through_without_buffering() {
        // An inline image (OSC 1337) spread over many packets.
        let mut data = b"\x1b]1337;File=inline=1:".to_vec();
        data.extend(std::iter::repeat_n(b'A', 256 * 1024));
        data.push(0x07);
        let mut pending = Vec::new();
        let mut replies = Vec::new();
        let mut output = Vec::new();
        for chunk in data.chunks(4096) {
            output.extend(
                process_ssh_output_for_ui(
                    chunk,
                    &mut pending,
                    &mut replies,
                    StandardDsrReply::SshLayer,
                )
                .await
                .expect("process chunk"),
            );
            assert!(pending.len() < MAX_DEFERRED_QUERY_BYTES);
        }
        assert!(replies.is_empty());
        assert_eq!(output, data);
    }

    #[tokio::test]
    async fn clipboard_capability_query_reports_osc_52() {
        for query in [b"\x1bP+q4d73\x1b\\", b"\x1bP+q4D73\x1b\\"] {
            let (output, replies, _) = process_in_chunks(query, 64).await;
            assert!(output.is_empty());
            assert_eq!(replies, XTGETTCAP_MS_REPLY.to_vec());
        }
    }

    #[tokio::test]
    async fn unterminated_dcs_query_is_released_past_the_cap() {
        let mut data = b"\x1bP+q".to_vec();
        data.extend(std::iter::repeat_n(b'4', MAX_DEFERRED_QUERY_BYTES));
        let (output, replies, pending) = process_in_chunks(&data, 16).await;
        assert!(replies.is_empty());
        assert!(pending.is_empty());
        assert_eq!(output, data);
    }
}
