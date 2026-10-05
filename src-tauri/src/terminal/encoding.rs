//! Character set conversion for remote shells that do not speak UTF-8
//! (GBK network gear, Big5 or Shift_JIS hosts). The webview only handles
//! UTF-8, so a session in another charset decodes its output here and encodes
//! typed text back before it reaches the server.

use encoding_rs::{CoderResult, Decoder, EncoderResult, Encoding, UTF_8};
use std::borrow::Cow;

/// Worst-case growth when decoding: every input byte can become a 3-byte
/// U+FFFD replacement character.
const DECODE_EXPANSION: usize = 3;

/// The charset a session's remote shell reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalEncoding(&'static Encoding);

impl Default for TerminalEncoding {
    fn default() -> Self {
        Self(UTF_8)
    }
}

impl TerminalEncoding {
    /// Resolves a WHATWG label such as `gbk` or `shift_jis`. A missing or
    /// unknown label is UTF-8, as are labels (UTF-16, `replacement`) whose
    /// encoder would write UTF-8 anyway.
    pub fn from_label(label: Option<&str>) -> Self {
        label
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .and_then(|label| Encoding::for_label(label.as_bytes()))
            .map(|encoding| Self(encoding.output_encoding()))
            .unwrap_or_default()
    }

    pub fn is_utf8(self) -> bool {
        self.0 == UTF_8
    }

    /// A streaming decoder for the session's output, or `None` when the
    /// output is UTF-8 already and passes through untouched.
    pub fn output_decoder(self) -> Option<OutputDecoder> {
        (!self.is_utf8()).then(|| OutputDecoder(self.0.new_decoder_without_bom_handling()))
    }

    /// Encodes typed or pasted text for the remote shell. `text` is UTF-8 from
    /// the webview; characters the charset cannot represent become `?`.
    pub fn encode_input(self, text: &[u8]) -> Cow<'_, [u8]> {
        if self.is_utf8() || text.is_ascii() {
            return Cow::Borrowed(text);
        }
        let text = String::from_utf8_lossy(text);
        let mut encoder = self.0.new_encoder();
        let mut output = Vec::with_capacity(text.len());
        let mut remaining: &str = &text;
        loop {
            let needed = encoder
                .max_buffer_length_from_utf8_without_replacement(remaining.len())
                .unwrap_or(remaining.len() * 4)
                .max(1);
            output.reserve(needed);
            let start = output.len();
            output.resize(start + needed, 0);
            let (result, read, written) =
                encoder.encode_from_utf8_without_replacement(remaining, &mut output[start..], true);
            output.truncate(start + written);
            remaining = &remaining[read..];
            match result {
                EncoderResult::InputEmpty => break,
                EncoderResult::OutputFull => {}
                EncoderResult::Unmappable(_) => output.push(b'?'),
            }
        }
        Cow::Owned(output)
    }
}

/// Converts one session's output stream to UTF-8. Keeps a multi-byte
/// character that a read split in two until its remaining bytes arrive.
pub struct OutputDecoder(Decoder);

impl OutputDecoder {
    pub fn decode(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut output = String::with_capacity(bytes.len() * DECODE_EXPANSION + 4);
        let mut remaining = bytes;
        loop {
            let (result, read, _) = self.0.decode_to_string(remaining, &mut output, false);
            remaining = &remaining[read..];
            match result {
                CoderResult::InputEmpty => break,
                CoderResult::OutputFull => output.reserve(remaining.len() * DECODE_EXPANSION + 4),
            }
        }
        output.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gbk() -> TerminalEncoding {
        TerminalEncoding::from_label(Some("gbk"))
    }

    #[test]
    fn missing_unknown_and_utf16_labels_are_utf8() {
        assert!(TerminalEncoding::from_label(None).is_utf8());
        assert!(TerminalEncoding::from_label(Some("")).is_utf8());
        assert!(TerminalEncoding::from_label(Some("no-such-charset")).is_utf8());
        assert!(TerminalEncoding::from_label(Some("utf-16le")).is_utf8());
        assert!(!gbk().is_utf8());
        assert!(!TerminalEncoding::from_label(Some(" Big5 ")).is_utf8());
    }

    #[test]
    fn utf8_sessions_skip_conversion() {
        let encoding = TerminalEncoding::default();
        assert!(encoding.output_decoder().is_none());
        assert!(matches!(
            encoding.encode_input("中文".as_bytes()),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn decodes_gbk_output_split_mid_character() {
        // "中文" in GBK is D6 D0 CE C4.
        let mut decoder = gbk().output_decoder().expect("gbk decoder");
        let mut decoded = decoder.decode(&[b'a', 0xd6]);
        decoded.extend(decoder.decode(&[0xd0, 0xce]));
        decoded.extend(decoder.decode(&[0xc4, b'\r', b'\n']));
        assert_eq!(String::from_utf8(decoded).unwrap(), "a中文\r\n");
    }

    #[test]
    fn escape_sequences_survive_decoding() {
        let mut decoder = gbk().output_decoder().expect("gbk decoder");
        let input = b"\x1b[31m\xd6\xd0\x1b[0m";
        assert_eq!(
            String::from_utf8(decoder.decode(input)).unwrap(),
            "\x1b[31m中\x1b[0m"
        );
    }

    #[test]
    fn encodes_input_and_replaces_unmappable_characters() {
        assert_eq!(
            gbk().encode_input("ls 中文".as_bytes()).as_ref(),
            b"ls \xd6\xd0\xce\xc4"
        );
        // An emoji has no GBK form.
        assert_eq!(gbk().encode_input("a😀b".as_bytes()).as_ref(), b"a?b");
        assert!(matches!(gbk().encode_input(b"plain"), Cow::Borrowed(_)));
    }

    #[test]
    fn encodes_long_input_across_buffer_refills() {
        let text = "中".repeat(10_000);
        let encoded = gbk().encode_input(text.as_bytes());
        assert_eq!(encoded.len(), 20_000);
        assert!(encoded.chunks(2).all(|pair| pair == [0xd6, 0xd0]));
    }
}
