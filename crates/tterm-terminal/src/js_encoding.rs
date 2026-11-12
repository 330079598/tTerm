//! The frontend's base64 and URI-component decoders.
//!
//! `atob` and `decodeURIComponent` are used for OSC payloads, so the ports
//! follow their forgiving-with-preconditions behaviour instead of Rust's
//! stricter decoders.

/// Decodes base64 and then UTF-8, as `decodeBase64Utf8` does in the frontend.
///
/// Invalid base64 returns `None`. Bytes that are not valid UTF-8 decode with
/// U+FFFD replacement characters, exactly like `TextDecoder` without
/// `{ fatal: true }`.
pub fn decode_base64_utf8(text: &str) -> Option<String> {
    let bytes = decode_base64(text)?;
    Some(decode_utf8_text(&bytes))
}

/// Decodes UTF-8 the way `new TextDecoder("utf-8")` does.
///
/// The decoder drops one leading byte-order mark and otherwise replaces
/// malformed sequences with U+FFFD instead of failing the whole input.
pub fn decode_utf8_text(bytes: &[u8]) -> String {
    let bytes = bytes
        .strip_prefix(b"\xEF\xBB\xBF".as_slice())
        .unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let mut input = Vec::with_capacity(text.len());
    for character in text.chars() {
        if matches!(character, '\t' | '\n' | '\u{000C}' | '\r' | ' ') {
            continue;
        }
        if !character.is_ascii() {
            return None;
        }
        input.push(character as u8);
    }

    // The HTML "forgiving-base64 decode" only removes padding when the input
    // length is a multiple of four. `atob("ab=")` throws, while `atob("ab")`
    // and `atob("YQ==")` decode.
    if input.len() % 4 == 0 {
        if input.ends_with(b"==") {
            input.truncate(input.len() - 2);
        } else if input.ends_with(b"=") {
            input.truncate(input.len() - 1);
        }
    }
    if input.contains(&b'=') || input.len() % 4 == 1 {
        return None;
    }

    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    for chunk in input.chunks(4) {
        let mut values = [0u8; 4];
        for (index, byte) in chunk.iter().enumerate() {
            values[index] = base64_value(*byte)?;
        }
        output.push((values[0] << 2) | (values[1] >> 4));
        if chunk.len() > 2 {
            output.push((values[1] << 4) | (values[2] >> 2));
        }
        if chunk.len() > 3 {
            output.push((values[2] << 6) | values[3]);
        }
    }
    Some(output)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Decodes a URI component the way `decodeURIComponent` does.
///
/// Percent escapes are decoded as UTF-8; malformed escapes and malformed
/// UTF-8 return `None`, which lets callers keep the raw payload.
pub fn decode_uri_component(text: &str) -> Option<String> {
    let mut output = String::with_capacity(text.len());
    let mut bytes = Vec::new();
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character == '%' {
            let high = characters.next()?;
            let low = characters.next()?;
            bytes.push(hex_value(high)? << 4 | hex_value(low)?);
            continue;
        }
        flush_utf8(&mut bytes, &mut output)?;
        output.push(character);
    }
    flush_utf8(&mut bytes, &mut output)?;
    Some(output)
}

fn flush_utf8(bytes: &mut Vec<u8>, output: &mut String) -> Option<()> {
    if bytes.is_empty() {
        return Some(());
    }
    output.push_str(std::str::from_utf8(bytes).ok()?);
    bytes.clear();
    Some(())
}

fn hex_value(character: char) -> Option<u8> {
    match character {
        '0'..='9' => Some(character as u8 - b'0'),
        'a'..='f' => Some(character as u8 - b'a' + 10),
        'A'..='F' => Some(character as u8 - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
