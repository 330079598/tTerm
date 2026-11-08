use super::*;

fn parse(input: &str) -> Result<Command, &'static str> {
    parse_request(input.as_bytes(), "secret", OperatingSystem::Windows)
}

#[test]
fn wrong_or_missing_tokens_never_authorize_a_command() {
    for token in ["", "secreT", "secret-extra", "secre"] {
        assert_eq!(
            parse(&format!("token={token}\ncommand=state\n")).unwrap_err(),
            "invalid token"
        );
    }
    assert!(!token_matches(b"bad", b"secret"));
    assert!(token_matches(b"secret", b"secret"));
}

#[test]
fn missing_command_and_duplicate_fields_are_rejected() {
    assert_eq!(parse("token=secret\n").unwrap_err(), "missing command");
    assert!(parse("token=secret\ntoken=secret\ncommand=state\n").is_err());
    assert!(parse("token=secret\ncommand=state\ncommand=screenshot\n").is_err());
}

#[test]
fn oversized_request_is_rejected_before_parsing() {
    let input = vec![b'a'; MAX_REQUEST_BYTES + 1];
    assert_eq!(
        parse_request(&input, "secret", OperatingSystem::Windows).unwrap_err(),
        "request exceeds 64 KiB"
    );
    assert_ne!(
        parse_request(
            &input[..MAX_REQUEST_BYTES],
            "secret",
            OperatingSystem::Windows
        )
        .unwrap_err(),
        "request exceeds 64 KiB"
    );
}

#[test]
fn text_preserves_unicode_spaces_and_punctuation_but_rejects_controls() {
    for text in ["中文 😀", "  $Host.UI.RawUI.WindowTitle = 'P0-23'  "] {
        let Command::Text(actual) = parse(&format!("token=secret\ncommand=text={text}\n")).unwrap()
        else {
            panic!("expected text")
        };
        assert_eq!(actual, text);
    }
    for text in ["x\ny", "x\ry", "x\ty", "\0", "\u{1b}", "\u{7f}", "\u{85}"] {
        assert!(validate_text(text).is_err());
        assert!(parse(&format!("token=secret\ncommand=text={text}\n")).is_err());
    }
    assert!(validate_text(&"中".repeat(1365)).is_ok());
    assert!(validate_text(&"中".repeat(1366)).is_err());
}

#[test]
fn keys_use_gpui_parser_including_letters_and_modifiers() {
    for key in [
        "a",
        "A",
        "ctrl-shift-x",
        "alt-enter",
        "ctrl--",
        "f12",
        "secondary-a",
    ] {
        let Command::Key(actual) = parse(&format!("token=secret\ncommand=key={key}\n")).unwrap()
        else {
            panic!("expected key")
        };
        assert_eq!(
            actual,
            Keystroke::parse_with_os(key, OperatingSystem::Windows).unwrap()
        );
    }
    for key in ["", "invalid-modifier-x", "a-b", "x\ny"] {
        assert!(validate_key(key, OperatingSystem::Windows).is_err());
    }
    assert!(validate_key(&"a".repeat(65), OperatingSystem::Windows).is_err());
}

#[test]
fn invalid_utf8_and_removed_toggle_are_rejected() {
    assert!(parse_request(&[0xff], "secret", OperatingSystem::Windows).is_err());
    assert!(parse("token=secret\ncommand=toggle-title\n").is_err());
    assert!(matches!(
        parse("token=secret\ncommand=state\n"),
        Ok(Command::State)
    ));
}
