use super::*;

#[test]
fn legacy_status_formatting_and_lossy_fallback_are_frozen() {
    assert_eq!(
        status_line("33", "Connecting"),
        "\r\n\x1b[33m[Connecting]\x1b[0m\r\n"
    );
    assert_eq!(
        jump_status_line("31", 2, "rejected"),
        "\r\n\x1b[31m[Jump #2: rejected]\x1b[0m\r\n"
    );
    assert_eq!(String::from_utf8_lossy(b"bad:\xff"), "bad:�");
}
