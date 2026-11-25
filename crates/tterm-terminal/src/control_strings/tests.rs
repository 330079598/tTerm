use super::ControlStringFramer;

/// The framer's output copied out of the buffers it borrows.
#[derive(Debug, PartialEq, Eq)]
struct Frame {
    code: Option<u16>,
    first_field: Vec<u8>,
    first_field_valid: bool,
    payload: Vec<u8>,
    payload_valid: bool,
    end: usize,
}

fn framer() -> ControlStringFramer {
    ControlStringFramer::with_osc52_payload(true)
}

fn frames(framer: &mut ControlStringFramer, input: &[u8]) -> Vec<Frame> {
    let mut completed = Vec::new();
    framer.scan(input, |event| {
        completed.push(Frame {
            code: event.code,
            first_field: event.first_field.to_vec(),
            first_field_valid: event.first_field_valid,
            payload: event.payload.to_vec(),
            payload_valid: event.payload_valid,
            end: event.end,
        });
    });
    completed
}

fn frames_in_chunks(mut framer: ControlStringFramer, input: &[u8], split: usize) -> Vec<Frame> {
    let mut completed = frames(&mut framer, &input[..split]);
    completed.extend(
        frames(&mut framer, &input[split..])
            .into_iter()
            .map(|mut frame| {
                frame.end += split;
                frame
            }),
    );
    completed
}

fn assert_split_equivalent(input: &[u8]) {
    let expected = frames(&mut framer(), input);
    for split in 0..=input.len() {
        assert_eq!(
            frames_in_chunks(framer(), input, split),
            expected,
            "split {split} of {input:?}"
        );
    }
}

const MIXED: &[u8] = b"plain \xc3\xa9 \x9b31m\x1b]0;title\x07\x1b]52;c;d\xc3\xa9\x1b\\\x1b[?25l\x1bP1$r0m\x1b\\\x1b]133;A\x07";

#[test]
fn mixed_control_and_string_packets_frame_the_same_across_a_split() {
    let completed = frames(&mut framer(), MIXED);
    assert_eq!(
        completed,
        vec![
            Frame {
                code: Some(0),
                first_field: b"title".to_vec(),
                first_field_valid: true,
                payload: Vec::new(),
                payload_valid: true,
                end: 23,
            },
            Frame {
                code: Some(52),
                first_field: b"c".to_vec(),
                first_field_valid: true,
                payload: b"c;d\xc3\xa9".to_vec(),
                payload_valid: true,
                end: 35,
            },
            Frame {
                code: Some(133),
                first_field: b"A".to_vec(),
                first_field_valid: true,
                payload: Vec::new(),
                payload_valid: true,
                end: 58,
            },
        ]
    );
    assert_split_equivalent(MIXED);
}

#[test]
fn utf8_in_ground_does_not_hide_escapes() {
    for input in [
        // Valid sequences.
        b"\xc3\xa9\x1b]0;x\x07".as_slice(),
        b"\xe4\xb8\xad\x1b]0;x\x07".as_slice(),
        // Truncated, surrogate and out-of-range sequences.
        b"\xc3\x1b]0;x\x07".as_slice(),
        b"\xf0\x9f\x1b]0;x\x07".as_slice(),
        b"\xed\xa0\x80\x1b]0;x\x07".as_slice(),
        b"\x80\xbf\xf5\x1b]0;x\x07".as_slice(),
    ] {
        let completed = frames(&mut framer(), input);
        assert_eq!(completed.len(), 1, "{input:?}");
        assert_eq!(completed[0].code, Some(0), "{input:?}");
        assert_eq!(completed[0].first_field, b"x".to_vec(), "{input:?}");
        assert_eq!(completed[0].end, input.len(), "{input:?}");
        assert_split_equivalent(input);
    }

    // A truncated lead byte at the end of a chunk leaves nothing to report.
    assert!(frames(&mut framer(), b"text\xc3").is_empty());
}

#[test]
fn raw_c1_introducers_stay_ignored_in_ground() {
    for input in [
        b"\x9b31m".as_slice(),
        b"\x9d52;c;QQ==\x07".as_slice(),
        b"\x90header\x9c".as_slice(),
        // 0x9B as the continuation of a valid UTF-8 sequence is still ignored.
        b"\xc2\x9b31m".as_slice(),
    ] {
        assert!(frames(&mut framer(), input).is_empty(), "{input:?}");
        assert_split_equivalent(input);
    }

    let input = b"\x9d52;c;QQ==\x07\x1b]52;c;QQ==\x07";
    let completed = frames(&mut framer(), input);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].code, Some(52));
    assert_eq!(completed[0].payload, b"c;QQ==".to_vec());
    assert_eq!(completed[0].end, input.len());
    assert_split_equivalent(input);
}

#[test]
fn payload_bytes_are_kept_verbatim_when_they_are_not_utf8() {
    let input = b"\x1b]52;c;\xc3(\xff\x07";
    let completed = frames(&mut framer(), input);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].first_field, b"c".to_vec());
    assert_eq!(completed[0].payload, b"c;\xc3(\xff".to_vec());
    assert_eq!(completed[0].end, input.len());
    assert_split_equivalent(input);
}

#[test]
fn string_endings_keep_their_offsets_across_splits() {
    for (input, end) in [
        (b"\x1b]0;x\x07".as_slice(), Some(6)),
        (b"\x1b]0;x\x1b\\".as_slice(), Some(7)),
        // CAN and SUB cancel a payload that has no terminator without reporting it.
        (b"\x1b]0;x\x18".as_slice(), None),
        // After an ESC, they close the string at the ESC instead.
        (b"\x1b]0;x\x1b\x18".as_slice(), Some(6)),
        (b"\x1b]0;x\x1b\x07".as_slice(), Some(6)),
        (b"\x1b]0;x\x1b\x1b\\".as_slice(), Some(6)),
    ] {
        let completed = frames(&mut framer(), input);
        assert_eq!(completed.len(), usize::from(end.is_some()), "{input:?}");
        if let Some(end) = end {
            assert_eq!(completed[0].end, end, "{input:?}");
        }
        assert_split_equivalent(input);
    }
}
