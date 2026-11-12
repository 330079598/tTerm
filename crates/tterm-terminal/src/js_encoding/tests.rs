use super::{decode_base64_utf8, decode_uri_component, decode_utf8_text};

#[test]
fn base64_matches_atob_and_utf8_decoding() {
    assert_eq!(decode_base64_utf8("YQ==").as_deref(), Some("a"));
    assert_eq!(decode_base64_utf8("YQ").as_deref(), Some("a"));
    assert_eq!(decode_base64_utf8("ab").as_deref(), Some("i"));
    assert_eq!(decode_base64_utf8("ab==").as_deref(), Some("i"));
    assert_eq!(decode_base64_utf8("abc=").as_deref(), Some("i\u{FFFD}"));
    assert_eq!(
        decode_base64_utf8("ab+c").as_deref(),
        Some("i\u{FFFD}\u{FFFD}")
    );
    assert_eq!(decode_base64_utf8("AA==").as_deref(), Some("\0"));
    assert_eq!(decode_base64_utf8("/w==").as_deref(), Some("\u{FFFD}"));
    assert_eq!(decode_base64_utf8("Y Q==").as_deref(), Some("a"));
    assert_eq!(decode_base64_utf8("YQ==\n").as_deref(), Some("a"));
}

#[test]
fn invalid_base64_is_rejected_like_atob() {
    for text in ["YQ=", "ab=", "a", "ab-c", "ab_c", "ab!", "ab===", "===="] {
        assert_eq!(decode_base64_utf8(text), None, "{text}");
    }
}

#[test]
fn empty_base64_decodes_to_an_empty_string() {
    assert_eq!(decode_base64_utf8("").as_deref(), Some(""));
    assert_eq!(decode_base64_utf8(" ").as_deref(), Some(""));
    assert_eq!(decode_base64_utf8("\n").as_deref(), Some(""));
}

#[test]
fn text_decoder_drops_one_leading_byte_order_mark() {
    assert_eq!(decode_utf8_text(&[0xEF, 0xBB, 0xBF, 0x61]), "a");
    assert_eq!(decode_utf8_text(&[0xEF, 0xBB, 0xBF]), "");
    assert_eq!(
        decode_utf8_text(&[0xEF, 0xBB, 0xBF, 0xEF, 0xBB, 0xBF]),
        "\u{FEFF}"
    );
    assert_eq!(decode_utf8_text(&[0x61, 0xEF, 0xBB, 0xBF]), "a\u{FEFF}");
    assert_eq!(decode_base64_utf8("77u/YQ==").as_deref(), Some("a"));
    assert_eq!(decode_base64_utf8("77u/").as_deref(), Some(""));
}

#[test]
fn uri_components_decode_utf8_percent_escapes() {
    assert_eq!(decode_uri_component("%C3%A9").as_deref(), Some("é"));
    assert_eq!(decode_uri_component("%E4%B8%AD").as_deref(), Some("中"));
    assert_eq!(decode_uri_component("%F0%9F%98%80").as_deref(), Some("😀"));
    assert_eq!(decode_uri_component("a%20b").as_deref(), Some("a b"));
    assert_eq!(decode_uri_component("%2F").as_deref(), Some("/"));
    assert_eq!(decode_uri_component("x%41y").as_deref(), Some("xAy"));
    assert_eq!(decode_uri_component("%C2%85").as_deref(), Some("\u{0085}"));
}

#[test]
fn malformed_uri_components_are_rejected() {
    for text in ["100%", "%", "%2", "%GG", "%C3", "%ED%A0%80"] {
        assert_eq!(decode_uri_component(text), None, "{text}");
    }
}
