use crate::workspace::local_shells::program_name;

#[test]
fn names_a_shell_after_its_program() {
    assert_eq!(program_name(r#""C:\Program Files\nu\nu.exe""#), "nu");
    assert_eq!(program_name("/opt/homebrew/bin/fish"), "fish");
    assert_eq!(program_name("pwsh"), "pwsh");
    assert_eq!(program_name(".hidden"), ".hidden");
}

#[test]
fn keeps_the_javascript_string_operations() {
    assert_eq!(program_name("nu .exe"), "nu ");
    assert_eq!(program_name("\"\"nu.exe\"\""), "\"nu");
    assert_eq!(program_name("  /bin/zsh  "), "zsh");
    assert_eq!(program_name("  'quoted'  "), "'quoted'");
    assert_eq!(program_name("nu."), "nu.");
    assert_eq!(program_name(""), "");
}
