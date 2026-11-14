//! Hex entry and display for serial tabs, from `src/lib/serial.ts`.

use super::SerialLineEnding;
use crate::js::is_js_whitespace;

/// Bytes as the hex view shows them: `AA 55 0D`.
pub fn format_hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// What a hex string turned into: its bytes, or the token that is not hex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedHex {
    Ok { bytes: Vec<u8> },
    Err { token: String },
}

fn token_separator(character: char) -> bool {
    is_js_whitespace(character) || matches!(character, ',' | ';' | ':' | '-')
}

/// The bytes a hex string names, the way people write them.
pub fn parse_hex_bytes(text: &str) -> ParsedHex {
    let mut bytes = Vec::new();
    for raw in text.split(token_separator).filter(|raw| !raw.is_empty()) {
        let token = raw
            .strip_prefix("0x")
            .or_else(|| raw.strip_prefix("0X"))
            .unwrap_or(raw);
        if token.is_empty()
            || token.len() % 2 != 0
            || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return ParsedHex::Err {
                token: raw.to_owned(),
            };
        }
        for [high, low] in token.as_bytes().as_chunks::<2>().0 {
            bytes.push(hex_value(*high) << 4 | hex_value(*low));
        }
    }
    ParsedHex::Ok { bytes }
}

fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

/// The bytes Enter sends with `ending`.
pub fn line_ending_bytes(ending: SerialLineEnding) -> Vec<u8> {
    match ending {
        SerialLineEnding::Cr => vec![0x0d],
        SerialLineEnding::Lf => vec![0x0a],
        SerialLineEnding::Crlf => vec![0x0d, 0x0a],
    }
}

#[cfg(test)]
mod tests {
    use super::{ParsedHex, line_ending_bytes, parse_hex_bytes};
    use crate::serial::SerialLineEnding;

    #[test]
    fn reads_hex_the_ways_people_write_it() {
        let expected = ParsedHex::Ok {
            bytes: vec![0x48, 0x65, 0x6c],
        };
        assert_eq!(parse_hex_bytes("48 65 6C"), expected);
        assert_eq!(parse_hex_bytes("48,65,6c"), expected);
        assert_eq!(parse_hex_bytes("0x48 0x65 0x6C"), expected);
        assert_eq!(parse_hex_bytes(" 48656c "), expected);
        assert_eq!(parse_hex_bytes(""), ParsedHex::Ok { bytes: vec![] });
    }

    #[test]
    fn names_the_token_that_is_not_hex() {
        assert_eq!(
            parse_hex_bytes("48 6G"),
            ParsedHex::Err {
                token: "6G".to_owned()
            }
        );
        assert_eq!(
            parse_hex_bytes("48 656"),
            ParsedHex::Err {
                token: "656".to_owned()
            }
        );
    }

    #[test]
    fn sends_what_enter_sends() {
        assert_eq!(line_ending_bytes(SerialLineEnding::Cr), vec![0x0d]);
        assert_eq!(line_ending_bytes(SerialLineEnding::Lf), vec![0x0a]);
        assert_eq!(line_ending_bytes(SerialLineEnding::Crlf), vec![0x0d, 0x0a]);
    }
}
