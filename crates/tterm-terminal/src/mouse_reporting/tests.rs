use super::MousePromptScanner;

fn scan_in_chunks(input: &[u8], split: usize) -> Vec<usize> {
    let mut scanner = MousePromptScanner::default();
    let mut offsets = scanner.scan(&input[..split]);
    offsets.extend(
        scanner
            .scan(&input[split..])
            .into_iter()
            .map(|offset| offset + split),
    );
    offsets
}

#[test]
fn a_prompt_offset_is_reported_immediately() {
    let input = b"x\x1b]133;A\x07tail";
    let mut scanner = MousePromptScanner::default();
    assert_eq!(scanner.scan(input), vec![9]);
}

#[test]
fn osc_633_and_st_are_recognized() {
    let input = b"\x1b]633;A\x1b\\";
    let mut scanner = MousePromptScanner::default();
    assert_eq!(scanner.scan(input), vec![input.len()]);
}

#[test]
fn only_type_a_marks_are_prompts() {
    let mut scanner = MousePromptScanner::default();
    assert!(scanner.scan(b"\x1b]133;C\x07").is_empty());
    assert!(scanner.scan(b"\x1b]133;D;1\x1b\\").is_empty());
    assert!(scanner.scan(b"\x1b]633;B\x07").is_empty());
    assert!(scanner.scan(b"\x1b]1337;File=inline=1:AAAA\x07").is_empty());
    assert_eq!(
        scanner.scan(b"\x1b]133;A;aid=1\x07"),
        vec![b"\x1b]133;A;aid=1\x07".len()]
    );
}

#[test]
fn bel_and_st_can_be_split_at_every_byte() {
    for input in [b"\x1b]133;A\x07".as_slice(), b"\x1b]633;A\x1b\\".as_slice()] {
        for split in 0..=input.len() {
            assert_eq!(
                scan_in_chunks(input, split),
                vec![input.len()],
                "split {split}"
            );
        }
    }
}

#[test]
fn a_prompt_before_trailing_bytes_reports_only_its_own_mark_end() {
    let input = b"prefix\x1b]633;A\x1b\\suffix";
    let end = b"prefix\x1b]633;A\x1b\\".len();
    for split in 0..=input.len() {
        assert_eq!(scan_in_chunks(input, split), vec![end], "split {split}");
    }
}

#[test]
fn st_is_not_reported_between_esc_and_backslash() {
    let input = b"\x1b]133;A\x1b\\\x1b[?1000h";
    let split = b"\x1b]133;A\x1b".len();
    let mut scanner = MousePromptScanner::default();
    assert!(scanner.scan(&input[..split]).is_empty());
    assert_eq!(scanner.scan(&input[split..]), vec![1]);
    assert!(scanner.scan(b"\x1b[?1003h").is_empty());
}

#[test]
fn a_prompt_followed_by_a_mode_enable_keeps_its_later_bytes() {
    let input = b"\x1b]133;A\x07\x1b[?1000h\x1b[?1003h";
    let mut scanner = MousePromptScanner::default();
    assert_eq!(scanner.scan(input), vec![8]);
    assert!(scanner.scan(b"\x1b[?1002h").is_empty());
}

#[test]
fn an_esc_os_after_dcs_or_apc_is_a_real_osc() {
    let input = b"\x1bP1;2|payload\x1b]133;A\x07";
    let mut scanner = MousePromptScanner::default();
    assert_eq!(scanner.scan(input), vec![input.len()]);

    let input = b"\x1b_payload\x1b]633;A\x1b\\";
    let mut scanner = MousePromptScanner::default();
    assert_eq!(scanner.scan(input), vec![input.len()]);
}

#[test]
fn raw_c1_inside_a_control_string_is_not_an_introducer_or_mark_end() {
    let input = b"\x1bPpayload\x9d133;A\x07still dcs";
    let mut scanner = MousePromptScanner::default();
    assert!(scanner.scan(input).is_empty());
    assert!(scanner.scan(b"\x9c").is_empty());
    assert_eq!(
        scanner.scan(b"\x1b]133;A\x07"),
        vec![b"\x1b]133;A\x07".len()]
    );
}

#[test]
fn an_esc_os_inside_an_osc_is_a_real_osc() {
    let input = b"\x1b]0;title\x1b]133;A\x07";
    let mut scanner = MousePromptScanner::default();
    assert_eq!(scanner.scan(input), vec![input.len()]);
}

#[test]
fn can_and_sub_abort_without_a_prompt() {
    for abort in [0x18, 0x1A] {
        let mut scanner = MousePromptScanner::default();
        let mut input = b"\x1b]133;A".to_vec();
        input.push(abort);
        assert!(scanner.scan(&input).is_empty());
        assert_eq!(
            scanner.scan(b"\x1b]133;A\x07"),
            vec![b"\x1b]133;A\x07".len()]
        );
    }
}

#[test]
fn raw_c1_introducers_in_ground_are_ignored() {
    let mut scanner = MousePromptScanner::default();
    assert!(scanner.scan(b"\x9d133;A\x9c").is_empty());
    assert_eq!(
        scanner.scan(b"\x1b]133;A\x07"),
        vec![b"\x1b]133;A\x07".len()]
    );
}

#[test]
fn raw_c1_st_inside_an_osc_is_payload() {
    let mut scanner = MousePromptScanner::default();
    assert!(scanner.scan(b"\x1b]133;A\x9c").is_empty());
    assert!(scanner.scan(b"\x07").is_empty());
    assert_eq!(
        scanner.scan(b"\x1b]133;A\x07"),
        vec![b"\x1b]133;A\x07".len()]
    );
}

#[test]
fn valid_utf8_continuations_are_not_controls() {
    // U+009D encoded as C2 9D must not open an OSC string, and a raw C1 byte
    // is invalid input rather than a sequence introducer.
    let mut scanner = MousePromptScanner::default();
    assert!(scanner.scan(b"\xc2\x9d133;A\x9c").is_empty());
    assert!(scanner.scan("\u{4f60}text".as_bytes()).is_empty());
    assert_eq!(
        scanner.scan(b"\x1b]133;A\x07"),
        vec![b"\x1b]133;A\x07".len()]
    );
}

#[test]
fn a_utf8_sequence_can_be_split_across_chunks() {
    let text = "\u{4f60}\u{597d}";
    let bytes = text.as_bytes();
    for split in 0..=bytes.len() {
        let mut scanner = MousePromptScanner::default();
        scanner.scan(&bytes[..split]);
        scanner.scan(&bytes[split..]);
        assert!(scanner.scan(b"\xb0text").is_empty());
    }
}

#[test]
fn an_oversized_osc_first_field_becomes_invalid_without_growing() {
    let mut scanner = MousePromptScanner::default();
    scanner.scan(b"\x1b]133;");
    let payload = vec![b'A'; 4096];
    assert!(scanner.scan(&payload).is_empty());
    assert!(scanner.scan(b"\x1b\\").is_empty());
    assert_eq!(
        scanner.scan(b"\x1b]133;A\x07"),
        vec![b"\x1b]133;A\x07".len()]
    );
}

#[test]
fn long_prompt_attributes_do_not_hide_the_prompt_mark() {
    let mut scanner = MousePromptScanner::default();
    let mut input = b"\x1b]133;A;aid=1;".to_vec();
    input.extend(std::iter::repeat_n(b'x', 4096));
    input.push(0x07);
    assert_eq!(scanner.scan(&input), vec![input.len()]);
}

#[test]
fn an_unfinished_control_string_is_forgotten_at_its_esc_end() {
    let mut scanner = MousePromptScanner::default();
    let mut input = b"\x1bP".to_vec();
    input.extend(std::iter::repeat_n(b'x', 4096));
    assert!(scanner.scan(&input).is_empty());
    assert_eq!(
        scanner.scan(b"\x1b]133;A\x07"),
        vec![b"\x1b]133;A\x07".len()]
    );
}

// The oracle is test-only and deliberately unbounded. Production scanning
// stays bounded; this harness only proves that its accepted prompt marks
// follow the real VTE transitions on a small finite table.
use alacritty_terminal::vte::{Parser, Perform};

use crate::command_mark::CommandMark;
use crate::command_marks::parse_command_mark;

struct VteOracle {
    fed: usize,
    current: u8,
    pending_esc: bool,
    prompts: Vec<usize>,
}

impl Perform for VteOracle {
    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        // The product policy, and the scanner, treat CAN/SUB as an abort:
        // no prompt mark is dispatched for them.
        if matches!(self.current, 0x18 | 0x1A) {
            return;
        }
        if params.len() < 2 || !matches!(params[0], b"133" | b"633") {
            return;
        }
        if std::str::from_utf8(params[1])
            .ok()
            .and_then(parse_command_mark)
            != Some(CommandMark::Prompt)
        {
            return;
        }
        if bell_terminated {
            self.prompts.push(self.fed);
        } else if self.current == 0x1B {
            // VTE dispatches an OSC when ESC arrives, before it knows whether
            // the next byte completes ST. Report the split-safe offset only
            // after the next byte.
            self.pending_esc = true;
        } else {
            self.prompts.push(self.fed);
        }
    }
}

fn oracle_prompts(input: &[u8]) -> Vec<usize> {
    let mut parser = Parser::new();
    let mut oracle = VteOracle {
        fed: 0,
        current: 0,
        pending_esc: false,
        prompts: Vec::new(),
    };
    for &byte in input {
        if oracle.pending_esc {
            if byte == b'\\' {
                oracle.prompts.push(oracle.fed + 1);
            } else {
                oracle.prompts.push(oracle.fed);
            }
            oracle.pending_esc = false;
        }
        oracle.current = byte;
        oracle.fed += 1;
        parser.advance(&mut oracle, &[byte]);
    }
    oracle.prompts
}

fn assert_matches_oracle(input: &[u8]) {
    let mut scanner = MousePromptScanner::default();
    assert_eq!(
        scanner.scan(input),
        oracle_prompts(input),
        "input {input:?}"
    );
    for split in 0..=input.len() {
        assert_eq!(
            scan_in_chunks(input, split),
            oracle_prompts(input),
            "split {split} of {input:?}"
        );
    }
}

#[test]
fn the_scanner_matches_the_real_vte_prompt_oracle() {
    let cases: &[&[u8]] = &[
        b"\x1b]133;A\x07",
        b"\x1b]633;A\x1b\\",
        b"\x1b]133;A;aid=1\x07",
        b"\x1b]133;A\x18",
        b"\x1b]133;A\x1a",
        b"\x1b]133;A\x1bX",
        b"\x1b \x5d133;A\x07",
        b"\x1b]13\x013;A\x07",
        b"\x1b]133;\x01A\x07",
        b"\x9d133;A\x9c",
        b"\x1b]133;A\x9c\x07",
        b"\x1bPpayload\x9cstill\x1b]133;A\x07",
        b"\x1b_apc\x1b]633;A\x1b\\",
        b"\x1bPdcs\x1b]133;A\x07",
        b"\x1bXstart of sos\x1b]133;A\x07",
        b"\x1b^pm\x1b]133;A\x07",
        b"\x1b]0;title\x1b]133;A\x07",
        b"\x1b]133;A\x1b\\x1b[?1000h",
    ];
    for input in cases {
        assert_matches_oracle(input);
    }
}
