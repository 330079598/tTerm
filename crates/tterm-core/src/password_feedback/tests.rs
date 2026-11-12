use super::next_typed_length;

#[test]
fn counts_each_typed_character_pasted_ones_included() {
    assert_eq!(next_typed_length(0, "s"), Some(1));
    assert_eq!(next_typed_length(1, "ecret"), Some(6));
    assert_eq!(next_typed_length(0, "密码🔑"), Some(3));
}

#[test]
fn erases_with_backspace_and_clears_with_ctrl_u() {
    assert_eq!(next_typed_length(3, "\x7f"), Some(2));
    assert_eq!(next_typed_length(3, "\u{0008}"), Some(2));
    assert_eq!(next_typed_length(0, "\x7f"), Some(0));
    assert_eq!(next_typed_length(5, "\x15ab"), Some(2));
}

#[test]
fn ignores_other_control_characters() {
    assert_eq!(next_typed_length(2, "\t\x04"), Some(2));
}

#[test]
fn ends_at_enter_or_ctrl_c() {
    assert_eq!(next_typed_length(4, "\r"), None);
    assert_eq!(next_typed_length(4, "pw\n"), None);
    assert_eq!(next_typed_length(4, "\x03"), None);
}

#[test]
fn counts_code_points_and_leaves_empty_input_alone() {
    assert_eq!(next_typed_length(3, ""), Some(3));
    assert_eq!(next_typed_length(-1, "a"), Some(0));
    assert_eq!(next_typed_length(0, "e\u{0301}"), Some(2));
}
