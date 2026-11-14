use serde_json::Value;

use super::split_shell_commands;

fn split(lines: &[&str]) -> Vec<String> {
    split_shell_commands(&lines.join("\n"))
}

fn assert_commands(actual: Vec<String>, expected: &[&str]) {
    let expected: Vec<String> = expected.iter().map(|line| (*line).to_owned()).collect();
    assert_eq!(actual, expected);
}

fn oracle() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/gpui-migration/reports/P1-11c-evidence/node-macos.json"
    )))
    .expect("the P1-11c oracle is valid JSON")
}

#[test]
fn keeps_each_plain_line_apart_and_drops_blank_ones() {
    assert_commands(
        split(&["cd /tmp", "", "  ", "ls -la # list", ""]),
        &["cd /tmp", "ls -la # list"],
    );
}

#[test]
fn keeps_a_line_ending_in_a_backslash_with_the_next() {
    assert_commands(
        split(&["docker run \\", "  -it alpine", "echo done"]),
        &["docker run \\\n  -it alpine", "echo done"],
    );
}

#[test]
fn an_escaped_backslash_ends_the_line() {
    assert_commands(split(&["echo a\\\\", "pwd"]), &["echo a\\\\", "pwd"]);
}

#[test]
fn keeps_an_open_quote_with_the_lines_up_to_its_end() {
    assert_commands(
        split(&["echo 'one", "", "two' \"x", "y\"", "pwd"]),
        &["echo 'one\n\ntwo' \"x\ny\"", "pwd"],
    );
}

#[test]
fn a_quote_inside_a_quote() {
    assert_commands(split(&["echo \"it's\"", "pwd"]), &["echo \"it's\"", "pwd"]);
}

#[test]
fn keeps_a_here_document_with_its_body_up_to_the_line_that_ends_it() {
    assert_commands(
        split(&["cat <<'EOF' > a.txt", "if then", "", "EOF", "pwd"]),
        &["cat <<'EOF' > a.txt\nif then\n\nEOF", "pwd"],
    );
}

#[test]
fn a_here_document_that_strips_tabs() {
    assert_commands(
        split(&["cat <<-END", "\tx", "\tEND", "pwd"]),
        &["cat <<-END\n\tx\n\tEND", "pwd"],
    );
}

#[test]
fn a_here_string() {
    assert_commands(
        split(&["grep x <<< 'y'", "pwd"]),
        &["grep x <<< 'y'", "pwd"],
    );
}

#[test]
fn keeps_a_block_together_however_its_lines_are_laid_out() {
    assert_commands(
        split(&["for f in *.log", "do", "  gzip $f", "done", "ls"]),
        &["for f in *.log\ndo\n  gzip $f\ndone", "ls"],
    );
    assert_commands(
        split(&[
            "if [ -f a ]; then",
            "  echo a",
            "elif [ -f b ]; then",
            "  echo b",
            "fi",
            "ls",
        ]),
        &[
            "if [ -f a ]; then\n  echo a\nelif [ -f b ]; then\n  echo b\nfi",
            "ls",
        ],
    );
    assert_commands(
        split(&["while read l; do echo $l; done < f", "ls"]),
        &["while read l; do echo $l; done < f", "ls"],
    );
    assert_commands(
        split(&[
            "case \"$1\" in",
            "  a) echo a ;;",
            "  *) echo other ;;",
            "esac",
            "ls",
        ]),
        &[
            "case \"$1\" in\n  a) echo a ;;\n  *) echo other ;;\nesac",
            "ls",
        ],
    );
}

#[test]
fn keeps_braces_and_parentheses_that_span_lines_together() {
    assert_commands(
        split(&["greet() {", "  echo \"hi ${1}\"", "}", "greet"]),
        &["greet() {\n  echo \"hi ${1}\"\n}", "greet"],
    );
    assert_commands(
        split(&["(cd /tmp &&", "  ls)", "pwd"]),
        &["(cd /tmp &&\n  ls)", "pwd"],
    );
    assert_commands(
        split(&["if ($x) {", "  'yes'", "}", "Get-Date"]),
        &["if ($x) {\n  'yes'\n}", "Get-Date"],
    );
    assert_commands(
        split(&["for f in *.txt", "  echo $f", "end", "ls"]),
        &["for f in *.txt\n  echo $f\nend", "ls"],
    );
}

#[test]
fn takes_no_keyword_from_an_argument_a_quote_or_a_comment() {
    assert_commands(
        split(&["echo if for done", "grep 'while' f", "ls # if", "pwd"]),
        &["echo if for done", "grep 'while' f", "ls # if", "pwd"],
    );
}

#[test]
fn still_sends_a_command_left_open_at_the_end() {
    assert_commands(split(&["ls", "echo 'oops"]), &["ls", "echo 'oops"]);
}

#[test]
fn splits_crlf_and_cr_like_the_typescript_regex() {
    assert_commands(split_shell_commands("cd /tmp\r\nls\r"), &["cd /tmp", "ls"]);
    assert_commands(split_shell_commands("cd /tmp\rls\n"), &["cd /tmp", "ls"]);
}

#[test]
fn a_keyword_glued_to_a_quote_is_no_keyword() {
    assert_commands(split(&["if\"x\""]), &["if\"x\""]);
}

#[test]
fn a_here_document_body_runs_to_the_word_that_ends_it() {
    assert_commands(
        split(&["cat <<X", "if then", "X", "pwd"]),
        &["cat <<X\nif then\nX", "pwd"],
    );
}

#[test]
fn unicode_whitespace_breaks_words_without_hanging() {
    assert_commands(
        split_shell_commands("echo a\u{00a0}b\u{feff}c"),
        &["echo a\u{00a0}b\u{feff}c"],
    );
}

#[test]
fn oracle_matches_every_typescript_shell_commands_case() {
    let oracle = oracle();
    let cases = oracle["cases"].as_array().expect("cases");
    let mut checked = 0;
    for case in cases {
        let id = case["id"].as_str().expect("id");
        if !id.starts_with("shellCommands.test") {
            continue;
        }
        let input: Vec<&str> = case["input"]
            .as_array()
            .expect("lines")
            .iter()
            .map(|line| line.as_str().expect("line"))
            .collect();
        let expected: Vec<String> = case["expected"]
            .as_array()
            .expect("expected lines")
            .iter()
            .map(|line| line.as_str().expect("line").to_owned())
            .collect();
        assert_eq!(split(&input), expected, "{id}");
        checked += 1;
    }
    assert_eq!(checked, 18, "all TypeScript cases were checked");

    let extra = &oracle["values"]["shellCommands.extra"];
    let crlf: Vec<&str> = extra["crlf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap())
        .collect();
    assert_eq!(split_shell_commands("cd /tmp\r\nls\r"), crlf);
    let glued: Vec<&str> = extra["gluedKeyword"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap())
        .collect();
    assert_eq!(split(&["if\"x\""]), glued);
    let heredoc: Vec<&str> = extra["heredocX"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap())
        .collect();
    assert_eq!(split(&["cat <<X"]), heredoc);
}
