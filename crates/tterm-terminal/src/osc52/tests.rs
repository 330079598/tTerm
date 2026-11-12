use super::{OSC52_CODE, parse_osc52_copy};

#[test]
fn names_the_osc52_sequence() {
    assert_eq!(OSC52_CODE, 52);
}

fn encode(text: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = text.as_bytes();
    let mut encoded = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0] as u32;
        let second = chunk.get(1).copied().unwrap_or(0) as u32;
        let third = chunk.get(2).copied().unwrap_or(0) as u32;
        let value = (first << 16) | (second << 8) | third;
        encoded.push(ALPHABET[((value >> 18) & 0x3f) as usize] as char);
        encoded.push(ALPHABET[((value >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(ALPHABET[((value >> 6) & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(ALPHABET[(value & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

#[test]
fn decodes_the_text_a_program_copies() {
    assert_eq!(
        parse_osc52_copy(&format!("c;{}", encode("hello world"))).as_deref(),
        Some("hello world")
    );
}

#[test]
fn decodes_utf8_text() {
    assert_eq!(
        parse_osc52_copy(&format!("c;{}", encode("你好，vim\n"))).as_deref(),
        Some("你好，vim\n")
    );
}

#[test]
fn accepts_any_selection_target_or_none() {
    let encoded = encode("a");
    assert_eq!(
        parse_osc52_copy(&format!(";{encoded}")).as_deref(),
        Some("a")
    );
    assert_eq!(
        parse_osc52_copy(&format!("p;{encoded}")).as_deref(),
        Some("a")
    );
    assert_eq!(
        parse_osc52_copy(&format!("s0;{encoded}")).as_deref(),
        Some("a")
    );
}

#[test]
fn never_answers_a_clipboard_query() {
    assert_eq!(parse_osc52_copy("c;?"), None);
}

#[test]
fn ignores_a_request_to_clear_the_clipboard() {
    assert_eq!(parse_osc52_copy("c;"), None);
}

#[test]
fn ignores_malformed_sequences() {
    assert_eq!(parse_osc52_copy("aGVsbG8="), None);
    assert_eq!(parse_osc52_copy("x;aGVsbG8="), None);
    assert_eq!(parse_osc52_copy("c;not base64!"), None);
}

#[test]
fn accepts_unpadded_base64_and_rejects_incomplete_padding() {
    assert_eq!(parse_osc52_copy("c;YQ").as_deref(), Some("a"));
    assert_eq!(parse_osc52_copy("c;YQ==").as_deref(), Some("a"));
    assert_eq!(parse_osc52_copy("c;YQ="), None);
}

#[test]
fn keeps_a_decoded_nul_character() {
    assert_eq!(parse_osc52_copy("c;AA==").as_deref(), Some("\0"));
}

#[test]
fn rejects_payloads_that_decode_to_empty_text() {
    assert_eq!(parse_osc52_copy("c;"), None);
    assert_eq!(parse_osc52_copy("c;===="), None);
}
