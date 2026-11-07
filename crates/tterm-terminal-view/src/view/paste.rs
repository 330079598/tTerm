//! Clipboard paste normalization, independent of platform clipboard access.

/// Encodes clipboard text without allowing an embedded bracket terminator.
///
/// Clipboard control characters are not keyboard gestures. Keep tabs and
/// newlines, remove the other C0/C1 controls, and normalize CRLF once.
pub(super) fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let filtered: String = normalized
        .chars()
        .filter(|ch| *ch == '\n' || *ch == '\t' || !ch.is_control())
        .collect();
    if filtered.is_empty() {
        return Vec::new();
    }
    if bracketed {
        [
            b"\x1b[200~".as_slice(),
            filtered.as_bytes(),
            b"\x1b[201~".as_slice(),
        ]
        .concat()
    } else {
        filtered.replace('\n', "\r").into_bytes()
    }
}
