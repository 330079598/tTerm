use super::*;

#[derive(Default)]
struct Observed {
    /// Display bytes and replies in the order the caller would receive them.
    stream: Vec<u8>,
    replies: Vec<Vec<u8>>,
    max_pending: usize,
}

impl Observed {
    fn record(&mut self, part: Part<'_>) {
        match part {
            Part::Display(bytes) => self.stream.extend_from_slice(bytes),
            Part::Reply(bytes) => {
                self.replies.push(bytes.to_vec());
                self.stream.extend_from_slice(bytes);
            }
        }
    }
}

fn feed_in_chunks(data: &[u8], chunk_size: usize) -> Observed {
    let mut queries = OrderedQueries::default();
    let mut observed = Observed::default();
    for chunk in data.chunks(chunk_size.max(1)) {
        queries.feed(chunk, |part| observed.record(part));
        observed.max_pending = observed.max_pending.max(queries.pending.len());
    }
    observed.max_pending = observed.max_pending.max(queries.pending.len());
    observed
}

#[test]
fn plain_text_is_borrowed_and_forwarded_in_one_piece() {
    let input: &[u8] = b"normal output\r\n\t\xe4\xb8\xad\xc3\x9c";
    let mut queries = OrderedQueries::default();
    let mut displays = 0;
    let mut seen = Vec::new();
    let mut borrowed = false;
    queries.feed(input, |part| match part {
        Part::Display(bytes) => {
            displays += 1;
            borrowed = bytes.as_ptr() == input.as_ptr() && bytes.len() == input.len();
            seen.extend_from_slice(bytes);
        }
        Part::Reply(_) => panic!("plain text must not trigger a reply"),
    });
    assert_eq!(displays, 1);
    assert!(borrowed, "plain text should not be copied");
    assert_eq!(seen, input);
    assert!(queries.pending.is_empty());
}

#[test]
fn multibyte_text_split_across_feeds_stays_unchanged() {
    let text = "中Ü😀 tail\r\n";
    for chunk_size in 1..=text.len() {
        let observed = feed_in_chunks(text.as_bytes(), chunk_size);
        assert_eq!(observed.stream, text.as_bytes(), "chunk size {chunk_size}");
        assert!(observed.replies.is_empty());
        assert_eq!(observed.max_pending, 0);
    }
}

#[test]
fn ansi_heavy_output_is_coalesced_before_the_model_write() {
    let mut data = b"\x1b]2;title\x07\x1b[?25l".to_vec();
    for _ in 0..100 {
        data.extend_from_slice(b"\x1b[38;5;196mred\x1b[0m");
    }
    let mut queries = OrderedQueries::default();
    let mut display_parts = Vec::new();
    queries.feed(&data, |part| match part {
        Part::Display(bytes) => display_parts.push(bytes.to_vec()),
        Part::Reply(_) => panic!("no fixed query in this output"),
    });
    assert_eq!(display_parts, vec![data], "one model write per feed");
}

#[test]
fn a_feed_without_a_query_never_emits_more_than_one_display() {
    let mut data = b"\x1b]2;title\xc3\x9c\x07".to_vec();
    for _ in 0..20 {
        data.extend_from_slice(b"\x1b[31mred\x1b[0m");
    }
    for chunk_size in 1..=data.len() {
        let mut queries = OrderedQueries::default();
        for chunk in data.chunks(chunk_size) {
            let mut displays = 0;
            queries.feed(chunk, |part| match part {
                Part::Display(_) => displays += 1,
                Part::Reply(_) => panic!("no fixed query in this output"),
            });
            assert!(
                displays <= 1,
                "chunk size {chunk_size} split one feed into {displays} model writes"
            );
        }
    }
}

#[test]
fn a_reply_splits_display_at_the_query_not_at_each_sgr() {
    let before = b"\x1b[31mred\x1b[0m";
    let after = b"\x1b[32mgreen\x1b[0m";
    let data = [before.as_slice(), b"\x1b[?6n", after].concat();
    let mut queries = OrderedQueries::default();
    let mut display_parts = Vec::new();
    let mut replies = Vec::new();
    queries.feed(&data, |part| match part {
        Part::Display(bytes) => display_parts.push(bytes.to_vec()),
        Part::Reply(bytes) => replies.push(bytes.to_vec()),
    });
    assert_eq!(display_parts, vec![before.to_vec(), after.to_vec()]);
    assert_eq!(replies, vec![DEC_CPR_REPLY.to_vec()]);
}

#[test]
fn packet_boundaries_do_not_change_the_order_of_display_and_fixed_replies() {
    let data = b"a\x1b[5n\x1b[?c\x1b[6n\x1bP+q4D73\x1b\\\x1b[?6n\x1b[>0c\x1bP+q6162\x9cb";
    let expected = [
        b"a\x1b[5n".as_slice(),
        DA1_REPLY,
        b"\x1b[6n",
        super::super::XTGETTCAP_MS_REPLY,
        DEC_CPR_REPLY,
        DA2_REPLY,
        b"\x1bP0+r6162\x1b\\",
        b"b",
    ]
    .concat();
    for chunk_size in 1..=data.len() {
        let observed = feed_in_chunks(data, chunk_size);
        assert_eq!(observed.stream, expected, "chunk size {chunk_size}");
        assert_eq!(observed.replies.len(), 5, "chunk size {chunk_size}");
        assert!(observed.max_pending <= MAX_DEFERRED_QUERY_BYTES);
    }
}

#[test]
fn nonquery_bytes_are_preserved_around_fixed_replies() {
    let data = b"\x1b[31mred\x1b[0m\x1b[?c middle \x1b[2J\x1b[>0cend";
    let expected = [
        b"\x1b[31mred\x1b[0m".as_slice(),
        DA1_REPLY,
        b" middle \x1b[2J",
        DA2_REPLY,
        b"end",
    ]
    .concat();
    for chunk_size in 1..=data.len() {
        let observed = feed_in_chunks(data, chunk_size);
        assert_eq!(observed.stream, expected, "chunk size {chunk_size}");
        assert_eq!(observed.replies.len(), 2, "chunk size {chunk_size}");
    }
}

#[test]
fn control_strings_end_on_raw_st_and_esc_backslash() {
    let strings: [(&[u8], &[u8]); 9] = [
        (b"\x1b]2;osc", b"\x1b\\"),
        (b"\x1b]2;osc", b"\x9c"),
        (b"\x1bPqdcs", b"\x1b\\"),
        (b"\x1bPqdcs", b"\x9c"),
        (b"\x1bP", b"\x9c"),
        (b"\x1bXsos", b"\x9c"),
        (b"\x1b^pm", b"\x9c"),
        (b"\x1b_apc", b"\x9c"),
        (b"\x1b]2;osc", b"\x07"),
    ];
    for (open, terminator) in strings {
        let mut data = open.to_vec();
        data.extend_from_slice(terminator);
        data.extend_from_slice(b"after\x1b[c");
        let mut expected = data[..data.len() - 3].to_vec();
        expected.extend_from_slice(DA1_REPLY);
        for chunk_size in 1..=data.len() {
            let observed = feed_in_chunks(&data, chunk_size);
            assert_eq!(
                observed.stream, expected,
                "{open:?} chunk size {chunk_size}"
            );
            assert_eq!(
                observed.replies.len(),
                1,
                "{open:?} chunk size {chunk_size}"
            );
            assert!(observed.max_pending <= MAX_DEFERRED_QUERY_BYTES);
        }
    }
}

#[test]
fn osc_boundaries_survive_escaped_payload_bytes() {
    let payloads: [&[u8]; 3] = [
        b"\x1b]2;a\x1bxb\x07",
        b"\x1b]2;a\x1b\x07",
        b"\x1b]2;a\x1b\x18b\x07",
    ];
    for payload in payloads {
        let mut data = payload.to_vec();
        data.extend_from_slice(b"\x1b[c");
        let mut expected = payload.to_vec();
        expected.extend_from_slice(DA1_REPLY);
        for chunk_size in 1..=data.len() {
            let observed = feed_in_chunks(&data, chunk_size);
            assert_eq!(
                observed.stream, expected,
                "payload {payload:?} chunk size {chunk_size}"
            );
            assert_eq!(observed.replies.len(), 1, "chunk size {chunk_size}");
        }
    }
}

#[test]
fn a_utf8_continuation_byte_is_not_a_raw_osc_terminator() {
    // "Ü" is C3 9C. The ESC [ c after it is title payload, not a query.
    let mut data = b"\x1b]2;\xc3\x9c\x1b[c".to_vec();
    data.push(0x07);
    data.extend_from_slice(b"X\x1b[c");
    let mut expected = data[..data.len() - 3].to_vec();
    expected.extend_from_slice(DA1_REPLY);
    for chunk_size in 1..=data.len() {
        let observed = feed_in_chunks(&data, chunk_size);
        assert_eq!(observed.stream, expected, "chunk size {chunk_size}");
        assert_eq!(observed.replies.len(), 1, "chunk size {chunk_size}");
    }
}

#[test]
fn a_utf8_continuation_byte_is_not_a_raw_dcs_terminator() {
    let mut data = b"\x1bPq\xc3\x9c\x1b[c".to_vec();
    data.push(0x9c);
    data.extend_from_slice(b"X\x1b[c");
    let mut expected = data[..data.len() - 3].to_vec();
    expected.extend_from_slice(DA1_REPLY);
    for chunk_size in 1..=data.len() {
        let observed = feed_in_chunks(&data, chunk_size);
        assert_eq!(observed.stream, expected, "chunk size {chunk_size}");
        assert_eq!(observed.replies.len(), 1, "chunk size {chunk_size}");
    }
}

#[test]
fn an_overlong_capability_is_released_without_retaining_its_payload() {
    let bytes = [b"\x1bP+q".as_slice(), &vec![b'1'; 16 * 1024], b"\x1b\\"].concat();
    for chunk_size in [1, 7, 80, 4096] {
        let observed = feed_in_chunks(&bytes, chunk_size);
        assert_eq!(observed.stream, bytes, "chunk size {chunk_size}");
        assert!(observed.replies.is_empty());
        assert!(observed.max_pending <= MAX_DEFERRED_QUERY_BYTES);
    }
}

#[test]
fn bel_does_not_end_a_dcs_or_an_apc_string() {
    for open in [
        b"\x1bPqdcs".as_slice(),
        b"\x1bXsos".as_slice(),
        b"\x1b_apc".as_slice(),
    ] {
        // The BEL and the DA-looking bytes are payload until raw ST ends it.
        let mut data = open.to_vec();
        data.extend_from_slice(b"\x07\x1b[c\x9c");
        data.extend_from_slice(b"\x1b[c");
        let mut expected = data[..data.len() - 3].to_vec();
        expected.extend_from_slice(DA1_REPLY);
        for chunk_size in 1..=data.len() {
            let observed = feed_in_chunks(&data, chunk_size);
            assert_eq!(
                observed.stream, expected,
                "{open:?} chunk size {chunk_size}"
            );
            assert_eq!(
                observed.replies.len(),
                1,
                "{open:?} chunk size {chunk_size}"
            );
        }
    }
}

#[test]
fn capability_pending_stays_within_the_deferred_bound() {
    let mut data = b"\x1bP+q".to_vec();
    data.extend(std::iter::repeat_n(b'4', MAX_DEFERRED_QUERY_BYTES - 4 - 1));
    let name = data[4..].to_vec();
    for (terminator, expected_name) in [
        (b"\x1b\\".as_slice(), name.as_slice()),
        (b"\x9c".as_slice(), name.as_slice()),
        (b"\x1b\x9c".as_slice(), name.as_slice()),
    ] {
        let mut query = data.clone();
        query.extend_from_slice(terminator);
        let expected = [b"\x1bP0+r".as_slice(), expected_name, b"\x1b\\"].concat();
        for chunk_size in 1..=query.len() {
            let observed = feed_in_chunks(&query, chunk_size);
            assert_eq!(observed.stream, expected, "chunk size {chunk_size}");
            assert_eq!(observed.replies, vec![expected.clone()]);
            assert!(
                observed.max_pending <= MAX_DEFERRED_QUERY_BYTES,
                "chunk size {chunk_size} held {} bytes",
                observed.max_pending
            );
        }
    }
}
