use super::*;
use crate::{
    grid::{CellSize, GridSize},
    terminal::{Config, Terminal},
};
use std::collections::HashMap;

fn palette() -> HighlightPalette {
    [
        ("background", "#000000"),
        ("foreground", "#ffffff"),
        ("cursor", "#ffffff"),
        ("selectionBackground", "#444444"),
        ("black", "#000000"),
        ("red", "#cc0000"),
        ("green", "#00cc00"),
        ("yellow", "#cccc00"),
        ("blue", "#0000cc"),
        ("magenta", "#cc00cc"),
        ("cyan", "#00cccc"),
        ("white", "#cccccc"),
        ("brightBlack", "#555555"),
        ("brightRed", "#ff5555"),
        ("brightGreen", "#55ff55"),
        ("brightYellow", "#ffff55"),
        ("brightBlue", "#5555ff"),
        ("brightMagenta", "#ff55ff"),
        ("brightCyan", "#55ffff"),
        ("brightWhite", "#ffffff"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect()
}
fn rule(overrides: impl FnOnce(&mut HighlightRule)) -> HighlightRule {
    let mut value = HighlightRule {
        id: "rule".into(),
        enabled: true,
        pattern: "error".into(),
        kind: "keyword".into(),
        case_sensitive: false,
        whole_word: false,
        foreground: Some("red".into()),
        background: None,
    };
    overrides(&mut value);
    value
}
fn set(rules: Vec<HighlightRule>) -> HighlightSet {
    HighlightSet {
        id: "set".into(),
        name: "Set".into(),
        rules,
    }
}
fn matched(text: &str, rules: Vec<HighlightRule>) -> Vec<String> {
    let spans = match_highlights(text, &compile_highlight_set(&set(rules), &palette()));
    spans
        .into_iter()
        .map(|span| {
            text.encode_utf16()
                .skip(span.start)
                .take(span.end - span.start)
                .collect::<Vec<_>>()
        })
        .map(|units| String::from_utf16(&units).unwrap())
        .collect()
}
fn builtin(text: &str) -> Vec<String> {
    matched(text, builtin_highlight_set().rules)
}

#[test]
fn js_whole_word_and_ascii_semantics() {
    assert_eq!(
        builtin("warnings successfully"),
        vec!["warnings", "successfully"]
    );
    assert_eq!(
        matched(
            "foo-barx",
            vec![rule(|r| {
                r.kind = "regex".into();
                r.pattern = "foo-bar|foo".into();
                r.whole_word = true;
            })]
        ),
        vec!["foo"]
    );
    assert_eq!(
        matched(
            "发生error了",
            vec![rule(|r| {
                r.kind = "regex".into();
                r.pattern = r"\berror\b".into();
            })]
        ),
        vec!["error"]
    );
    assert!(
        matched(
            "permission 拒绝",
            vec![rule(|r| {
                r.kind = "regex".into();
                r.pattern = r"permission \w+".into();
            })]
        )
        .is_empty()
    );
    assert_eq!(
        matched(
            "ſ S",
            vec![rule(|r| {
                r.kind = "regex".into();
                r.pattern = "s".into();
            })]
        ),
        vec!["S"]
    );
    assert_eq!(
        matched(
            "K K",
            vec![rule(|r| {
                r.kind = "regex".into();
                r.pattern = "k".into();
            })]
        ),
        vec!["K"]
    );
}
#[test]
fn js_lookaround_and_backreference_work() {
    assert_eq!(
        matched(
            "foo foo",
            vec![rule(|r| {
                r.kind = "regex".into();
                r.pattern = r"(foo) \1".into();
            })]
        ),
        vec!["foo foo"]
    );
    assert_eq!(
        matched(
            "a1",
            vec![rule(|r| {
                r.kind = "regex".into();
                r.pattern = r"(?<=a)1".into();
            })]
        ),
        vec!["1"]
    );
}
#[test]
fn builtin_address_rules_match_js_examples() {
    assert_eq!(builtin("Connected to 192.168.1.1."), vec!["192.168.1.1"]);
    assert_eq!(builtin("ssh 10.0.0.1:22"), vec!["10.0.0.1"]);
    assert_eq!(builtin("inet:10.0.0.1"), vec!["10.0.0.1"]);
    assert_eq!(builtin("10.0.0.0/33"), vec!["10.0.0.0"]);
    assert_eq!(builtin("2001:db8::1"), vec!["2001:db8::1"]);
    assert_eq!(builtin("::2"), vec!["::2"]);
    assert_eq!(builtin("aa:bb:cc:dd:ee:ff"), vec!["aa:bb:cc:dd:ee:ff"]);
    assert!(builtin("at 10:30 done").is_empty());
    assert!(builtin("1:2:3:4").is_empty());
}
#[test]
fn builtin_errors_warnings_successes() {
    assert_eq!(
        builtin("ERROR: connection refused, Failed twice"),
        vec!["ERROR", "refused", "Failed"]
    );
    assert_eq!(
        builtin("warning: request timed out"),
        vec!["warning", "timed out"]
    );
    assert_eq!(builtin("[ OK ] tests passed"), vec!["passed"]);
    assert!(builtin("errorlevel token failsafe").is_empty());
}
#[test]
fn builtin_chinese_boundary() {
    assert_eq!(builtin("发生error了"), vec!["error"]);
}
#[test]
fn builtin_ipv4() {
    assert_eq!(
        builtin("from 192.168.1.10 via 10.0.0.0/8"),
        vec!["192.168.1.10", "10.0.0.0/8"]
    );
    assert!(builtin("version 1.2.3.4.5 and 256.1.1.1").is_empty());
}
#[test]
fn builtin_ipv6() {
    assert_eq!(
        builtin("inet6 fe80::1/64 and 2001:db8:0:0:0:0:2:1 and ::1"),
        vec!["fe80::1/64", "2001:db8:0:0:0:0:2:1", "::1"]
    );
    assert!(builtin("at 12:30:45 Add::add std::cafe").is_empty());
}
#[test]
fn builtin_mac() {
    assert_eq!(
        builtin("ether 52:54:00:ab:cd:ef 52-54-00-AB-CD-EF 5254.00ab.cdef"),
        vec!["52:54:00:ab:cd:ef", "52-54-00-AB-CD-EF", "5254.00ab.cdef"]
    );
}
#[test]
fn keyword_literal() {
    assert_eq!(
        matched("a.b axb", vec![rule(|r| r.pattern = "a.b".into())]),
        vec!["a.b"]
    );
}
#[test]
fn case_and_whole_word() {
    assert_eq!(
        matched(
            "error Error Errors",
            vec![rule(|r| {
                r.pattern = "Error".into();
                r.case_sensitive = true;
                r.whole_word = true;
            })]
        ),
        vec!["Error"]
    );
}
#[test]
fn earlier_rule_wins() {
    let spans = match_highlights(
        "permission denied, permission granted",
        &compile_highlight_set(
            &set(vec![
                rule(|r| {
                    r.id = "a".into();
                    r.pattern = "denied".into()
                }),
                rule(|r| {
                    r.id = "b".into();
                    r.kind = "regex".into();
                    r.pattern = r"permission \w+".into();
                    r.foreground = Some("green".into());
                }),
            ]),
            &palette(),
        ),
    );
    assert_eq!(
        spans,
        vec![
            HighlightSpan {
                start: 11,
                end: 17,
                foreground: Some("#cc0000".into()),
                background: None
            },
            HighlightSpan {
                start: 19,
                end: 37,
                foreground: Some("#00cc00".into()),
                background: None
            }
        ]
    );
}
#[test]
fn skips_disabled_invalid_colorless() {
    let rules = vec![
        rule(|r| {
            r.pattern = "one".into();
            r.enabled = false
        }),
        rule(|r| {
            r.pattern = "(two".into();
            r.kind = "regex".into()
        }),
        rule(|r| {
            r.pattern = "three".into();
            r.foreground = None
        }),
        rule(|r| {
            r.pattern = "four".into();
            r.foreground = None;
            r.background = Some("#123456".into());
        }),
    ];
    assert_eq!(matched("one two three four", rules), vec!["four"]);
}
#[test]
fn caps_spans_and_text() {
    assert_eq!(
        matched(&("x ".repeat(100)), vec![rule(|r| r.pattern = "x".into())]).len(),
        MAX_SPANS_PER_LINE
    );
    assert!(
        matched(
            &format!("{}x", " ".repeat(MAX_HIGHLIGHT_TEXT)),
            vec![rule(|r| r.pattern = "x".into())]
        )
        .is_empty()
    );
}
#[test]
fn pattern_errors() {
    assert_eq!(
        highlight_pattern_error(&rule(|r| r.pattern = "  ".into())),
        Some("empty")
    );
    assert_eq!(
        highlight_pattern_error(&rule(|r| {
            r.pattern = "[a".into();
            r.kind = "regex".into()
        })),
        Some("invalid")
    );
    assert_eq!(
        highlight_pattern_error(&rule(|r| {
            r.pattern = "a*".into();
            r.kind = "regex".into()
        })),
        Some("matchesEmpty")
    );
    assert_eq!(
        highlight_pattern_error(&rule(|r| r.pattern = "[a".into())),
        None
    );
}
#[test]
fn resolve_palette_names_and_hex_colors() {
    assert_eq!(
        resolve_highlight_color(Some("brightRed"), &palette()),
        Some("#ff5555".into())
    );
    assert_eq!(
        resolve_highlight_color(Some("#ABC"), &palette()),
        Some("#aabbcc".into())
    );
    assert_eq!(resolve_highlight_color(Some("nonsense"), &palette()), None);
    assert_eq!(resolve_highlight_color(None, &palette()), None);
    let mut invalid = HashMap::new();
    invalid.insert("red".into(), "bad".into());
    assert_eq!(
        resolve_highlight_color(Some("red"), &invalid),
        Some("#cc0000".into())
    );
    assert_eq!(
        resolve_highlight_color(Some("rgb(1 2 3)"), &palette()),
        Some("#010203".into())
    );
    assert_eq!(
        resolve_highlight_color(Some("RGB(1,2,3)"), &palette()),
        Some("#010203".into())
    );
    assert_eq!(
        resolve_highlight_color(Some("hsl(0 100% 50%)"), &palette()),
        Some("#ff0000".into())
    );
    assert_eq!(resolve_highlight_color(Some(" red"), &palette()), None);
    assert_eq!(
        resolve_highlight_color(Some("red"), &HashMap::new()),
        Some("#cc0000".into())
    );
}
#[test]
fn resolve_uses_xterm_fallback_colors() {
    assert_eq!(
        resolve_highlight_color(Some("red"), &HashMap::new()),
        Some("#cc0000".into())
    );
}
fn terminal(columns: usize, lines: usize) -> Terminal {
    Terminal::new(
        Config::default(),
        GridSize::new(columns, lines),
        CellSize::new(8, 16),
    )
}
fn write(terminal: &Terminal, text: &str) {
    terminal.write(text.as_bytes());
}
#[test]
fn reads_wrapped_line_and_drops_blanks() {
    let term = terminal(10, 5);
    write(&term, "connect to 10.0.0.1");
    let cells = read_cell_text(&term.snapshot(), 0, 2, 2000);
    assert_eq!(cells.text, "connect to 10.0.0.1");
    assert_eq!(&cells.rows[9..12], [0, 1, 1]);
    assert_eq!(&cells.columns[9..12], [9, 0, 1]);
}
#[test]
fn maps_wide_characters() {
    let term = terminal(20, 5);
    write(&term, "错误 error");
    let cells = read_cell_text(&term.snapshot(), 0, 1, 2000);
    assert_eq!(cells.text, "错误 error");
    assert_eq!(cells.columns, vec![0, 2, 4, 5, 6, 7, 8, 9]);
    assert_eq!(&cells.widths[..3], [2, 2, 1]);
}
#[test]
fn marks_program_colored_cells() {
    let term = terminal(20, 5);
    write(&term, "a\x1b[31mb\x1b[0m\x1b[7mc\x1b[0m\x1b[1md");
    assert_eq!(
        read_cell_text(&term.snapshot(), 0, 1, 2000).plain,
        vec![true, false, false, true]
    );
}
#[test]
fn stops_at_length() {
    let term = terminal(10, 5);
    write(&term, &"x".repeat(25));
    assert_eq!(
        read_cell_text(&term.snapshot(), 0, 3, 12)
            .text
            .encode_utf16()
            .count(),
        12
    );
}
#[test]
fn truncates_input_by_utf16_units() {
    let text = format!("{}x", "😀".repeat(1000));
    let rules = compile_highlight_set(&set(vec![rule(|r| r.pattern = "x".into())]), &palette());
    assert!(match_highlights(&text, &rules).is_empty());
}
#[test]
fn keeps_space_before_combining_character_and_tabs_as_spaces() {
    let term = terminal(20, 2);
    write(&term, " \u{301}\tX");
    let cells = read_cell_text(&term.snapshot(), 0, 1, 2000);
    assert_eq!(cells.text, " \u{301}       X");
    assert_eq!(cells.columns, vec![0, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
}
#[test]
fn highlight_wrap_segments() {
    let term = terminal(10, 5);
    write(&term, "connect to 10.0.0.1");
    let cells = read_cell_text(&term.snapshot(), 0, 2, 2000);
    assert_eq!(
        highlight_segments(
            &cells,
            &[HighlightSpan {
                start: 11,
                end: 19,
                foreground: Some("#00cccc".into()),
                background: None
            }]
        ),
        vec![HighlightSegment {
            row: 1,
            x: 1,
            width: 8,
            foreground: Some("#00cccc".into()),
            background: None
        }]
    );
    assert_eq!(
        highlight_segments(
            &cells,
            &[HighlightSpan {
                start: 8,
                end: 13,
                foreground: Some("#00cccc".into()),
                background: None
            }]
        ),
        vec![
            HighlightSegment {
                row: 0,
                x: 8,
                width: 2,
                foreground: Some("#00cccc".into()),
                background: None
            },
            HighlightSegment {
                row: 1,
                x: 0,
                width: 3,
                foreground: Some("#00cccc".into()),
                background: None
            }
        ]
    );
}
#[test]
fn highlight_wide_characters() {
    let term = terminal(20, 5);
    write(&term, "发生错误");
    let cells = read_cell_text(&term.snapshot(), 0, 1, 2000);
    assert_eq!(
        highlight_segments(
            &cells,
            &[HighlightSpan {
                start: 2,
                end: 4,
                foreground: Some("#cc0000".into()),
                background: None
            }]
        ),
        vec![HighlightSegment {
            row: 0,
            x: 4,
            width: 4,
            foreground: Some("#cc0000".into()),
            background: None
        }]
    );
}
#[test]
fn leaves_program_colored_cells() {
    let term = terminal(20, 5);
    write(&term, "err\x1b[31mor\x1b[0m!");
    let cells = read_cell_text(&term.snapshot(), 0, 1, 2000);
    assert_eq!(
        highlight_segments(
            &cells,
            &[HighlightSpan {
                start: 0,
                end: 6,
                foreground: Some("#cc0000".into()),
                background: None
            }]
        ),
        vec![
            HighlightSegment {
                row: 0,
                x: 0,
                width: 3,
                foreground: Some("#cc0000".into()),
                background: None
            },
            HighlightSegment {
                row: 0,
                x: 5,
                width: 1,
                foreground: Some("#cc0000".into()),
                background: None
            }
        ]
    );
}

#[test]
fn ucs2_matches_non_ascii_text_without_utf8_slicing() {
    for (pattern, text, expected) in [
        ("error.", "error中", vec![(0, 6)]),
        (r"\S", "é", vec![(0, 1)]),
        ("[^x]", "é", vec![(0, 1)]),
        (r"[\x80-\xff]", "é", vec![(0, 1)]),
        ("ошибка", "ОШИБКА", vec![(0, 6)]),
        (r"\s", "　", vec![(0, 1)]),
        ("s", "ſ S", vec![(2, 3)]),
        ("k", "K K", vec![(0, 1)]),
        (".", "😀", vec![(0, 1), (1, 2)]),
        ("(?=.)|x", "😀x", vec![]),
    ] {
        let rules = compile_highlight_set(
            &set(vec![rule(|value| {
                value.kind = "regex".into();
                value.pattern = pattern.into();
            })]),
            &palette(),
        );
        let spans = match_highlights(text, &rules);
        assert_eq!(
            spans
                .iter()
                .map(|span| (span.start, span.end))
                .collect::<Vec<_>>(),
            expected,
            "{pattern}: {text}"
        );
    }
}

#[test]
fn builtin_source_equals_the_real_typescript_set() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/gpui-migration/reports/P1-11b-evidence/node.json"
    ))
    .unwrap();
    assert_eq!(
        serde_json::to_value(builtin_highlight_set()).unwrap(),
        oracle["builtin"]
    );
}
