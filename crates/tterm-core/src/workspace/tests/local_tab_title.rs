use indexmap::IndexMap;

use crate::workspace::local_tab_title::{
    MAX_COMMAND_TITLE_LENGTH, command_title, directory_title, distinct_local_titles,
    has_legacy_local_title, local_shell_title, running_command_title, tab_tooltip,
    with_distinct_local_titles, with_local_cwd,
};
use crate::workspace::tab::{ConnectionType, Tab, TabConnection, TabType, TerminalShellType};

use super::oracle_11d2;

fn tab(id: &str, title: &str, fields: impl FnOnce(&mut Tab)) -> Tab {
    let mut tab = Tab::new(id, title, TabType::Terminal);
    fields(&mut tab);
    tab
}

fn connection() -> TabConnection {
    let mut connection = TabConnection::default();
    connection.set_connection_type(Some(ConnectionType::Terminal));
    connection
}

fn shell_connection(shell: TerminalShellType) -> TabConnection {
    let mut connection = connection();
    connection.set_terminal_shell(Some(shell));
    connection
}

fn local_tab(id: &str, cwd: &str, fields: impl FnOnce(&mut Tab)) -> Tab {
    tab(id, "", |tab| {
        tab.set_auto_title(Some(true));
        let mut connection = connection();
        connection.set_cwd(Some(cwd.to_string()));
        tab.set_connection(Some(connection));
        fields(tab);
    })
}

fn titles(tabs: &[Tab], home: Option<&str>) -> Vec<String> {
    distinct_local_titles(tabs, home)
        .into_iter()
        .map(|tab| tab.title.clone())
        .collect()
}

#[test]
fn recognizes_the_old_default_names_of_local_tabs_only() {
    for (title, expected) in [
        ("OS terminal", true),
        ("OS terminal-3", true),
        ("本地终端", true),
        ("builds", false),
    ] {
        assert_eq!(
            has_legacy_local_title(&tab("1", title, |_| {})),
            expected,
            "{title}"
        );
    }
    let mut ssh = tab("1", "Terminal", |_| {});
    ssh.set_tab_type(TabType::Ssh);
    assert!(!has_legacy_local_title(&ssh));
}

#[test]
fn names_a_directory_by_its_last_part() {
    assert_eq!(
        directory_title("D:\\code\\rust\\tTerm", None).as_deref(),
        Some("tTerm")
    );
    assert_eq!(
        directory_title("/home/me/src/", None).as_deref(),
        Some("src")
    );
    assert_eq!(
        directory_title("\\\\server\\share", None).as_deref(),
        Some("share")
    );
}

#[test]
fn shows_the_home_directory_as_tilde_however_the_shell_spells_it() {
    let home = "C:\\Users\\Me";
    assert_eq!(
        directory_title("C:\\Users\\Me", Some(home)).as_deref(),
        Some("~")
    );
    assert_eq!(
        directory_title("c:/users/me/", Some(home)).as_deref(),
        Some("~")
    );
    assert_eq!(
        directory_title("/c/Users/Me", Some(home)).as_deref(),
        Some("~")
    );
    assert_eq!(
        directory_title("/home/me", Some("/home/me")).as_deref(),
        Some("~")
    );
    assert_eq!(
        directory_title("/home/me/src", Some("/home/me")).as_deref(),
        Some("src")
    );
}

#[test]
fn keeps_roots_whole() {
    assert_eq!(directory_title("/", Some("/home/me")).as_deref(), Some("/"));
    assert_eq!(directory_title("C:\\", None).as_deref(), Some("C:\\"));
    assert_eq!(directory_title("C:", None).as_deref(), Some("C:\\"));
    assert_eq!(directory_title("  ", None), None);
}

#[test]
fn shows_a_short_command_whole() {
    assert_eq!(command_title("npm run dev").as_deref(), Some("npm run dev"));
    assert_eq!(command_title("  ssh prod  ").as_deref(), Some("ssh prod"));
}

#[test]
fn shows_the_program_alone_for_a_long_command() {
    assert_eq!(
        command_title("cargo test --all-targets -- -D warnings").as_deref(),
        Some("cargo")
    );
}

#[test]
fn drops_the_programs_path_extension_and_what_runs_it() {
    assert_eq!(
        command_title("C:\\tools\\vim.exe notes.txt").as_deref(),
        Some("vim notes.txt")
    );
    assert_eq!(
        command_title("& \"C:\\Program Files\\node.exe\" app.js").as_deref(),
        Some("node app.js")
    );
    assert_eq!(command_title("& .\\build.ps1").as_deref(), Some("build"));
    assert_eq!(
        command_title("RUST_LOG=debug sudo /usr/bin/htop").as_deref(),
        Some("htop")
    );
}

#[test]
fn names_a_tab_named_automatically_after_its_directory() {
    let next = with_local_cwd(
        &tab("tab-1", "pwsh", |tab| tab.set_auto_title(Some(true))),
        "/work/tTerm",
        None,
    );
    assert_eq!(next.title, "tTerm");
    assert_eq!(
        next.connection
            .as_ref()
            .and_then(|connection| connection.cwd.as_deref()),
        Some("/work/tTerm")
    );
}

#[test]
fn keeps_the_name_the_user_gave() {
    let next = with_local_cwd(&tab("tab-1", "builds", |_| {}), "/work/tTerm", None);
    assert_eq!(next.title, "builds");
    assert_eq!(
        next.connection
            .as_ref()
            .and_then(|connection| connection.cwd.as_deref()),
        Some("/work/tTerm")
    );
}

#[test]
fn returns_the_same_tab_when_nothing_changes() {
    let current = tab("tab-1", "tTerm", |tab| {
        tab.set_auto_title(Some(true));
        let mut connection = connection();
        connection.set_cwd(Some("/work/tTerm".to_string()));
        tab.set_connection(Some(connection));
    });
    assert_eq!(with_local_cwd(&current, "/work/tTerm", None), current);
}

#[test]
fn adds_a_local_tabs_directory_or_a_remote_tabs_host() {
    assert_eq!(
        tab_tooltip(&tab("1", "builds", |tab| {
            let mut connection = connection();
            connection.set_cwd(Some("/work".to_string()));
            tab.set_connection(Some(connection));
        })),
        "builds (/work)"
    );
    assert_eq!(
        tab_tooltip(&tab("1", "pwsh", |tab| {
            tab.set_connection(Some(TabConnection::default()));
        })),
        "pwsh"
    );
    let mut ssh = tab("1", "prod", |tab| {
        let mut connection = TabConnection::default();
        connection.set_host(Some("10.0.0.1".to_string()));
        tab.set_connection(Some(connection));
    });
    ssh.set_tab_type(TabType::Ssh);
    assert_eq!(tab_tooltip(&ssh), "prod (10.0.0.1)");
}

#[test]
fn adds_parent_directories_until_same_named_directories_differ() {
    assert_eq!(
        titles(
            &[
                local_tab("1", "/work/web/src", |_| {}),
                local_tab("2", "/work/api/src", |_| {}),
                local_tab("3", "/home/me/api/src", |_| {}),
            ],
            None
        ),
        vec!["web/src", "work/api/src", "me/api/src"]
    );
}

#[test]
fn keeps_the_plain_name_for_tabs_in_the_very_same_directory() {
    let tabs = [
        local_tab("1", "D:\\code\\tTerm", |_| {}),
        local_tab("2", "d:/code/tTerm/", |_| {}),
    ];
    assert_eq!(titles(&tabs, None), vec!["tTerm", "tTerm"]);
}

#[test]
fn separates_windows_paths_with_backslashes() {
    assert_eq!(
        titles(
            &[
                local_tab("1", "C:\\a\\src", |_| {}),
                local_tab("2", "C:\\b\\src", |_| {}),
            ],
            None
        ),
        vec!["a\\src", "b\\src"]
    );
}

#[test]
fn falls_back_to_the_whole_path_when_one_directory_ends_the_other() {
    assert_eq!(
        titles(
            &[
                local_tab("1", "/src", |_| {}),
                local_tab("2", "/a/src", |_| {}),
            ],
            None
        ),
        vec!["/src", "a/src"]
    );
}

#[test]
fn leaves_tabs_the_user_named_and_returns_the_same_array_when_nothing_changes() {
    let tabs = [
        local_tab("1", "/a/src", |tab| tab.set_title("src".to_string())),
        local_tab("2", "/b/src", |tab| {
            tab.set_auto_title(None);
            tab.set_title("mine".to_string());
        }),
    ];
    assert_eq!(titles(&tabs, None), vec!["src", "mine"]);
    assert_eq!(distinct_local_titles(&tabs, None), tabs);
}

#[test]
fn shows_the_home_directory_as_tilde_in_distinct_titles() {
    assert_eq!(
        titles(&[local_tab("1", "/home/me", |_| {})], Some("/home/me")),
        vec!["~"]
    );
}

#[test]
fn names_the_shell_after_the_directory() {
    let tab = tab("1", "tTerm", |tab| {
        tab.set_shell_name(Some("pwsh".to_string()));
        let mut connection = connection();
        connection.set_cwd(Some("D:\\code\\tTerm".to_string()));
        tab.set_connection(Some(connection));
    });
    assert_eq!(tab_tooltip(&tab), "tTerm (D:\\code\\tTerm · pwsh)");
}

#[test]
fn names_a_wsl_shell_after_its_distribution() {
    let tab = tab("1", "~", |tab| {
        tab.set_shell_name(Some("WSL".to_string()));
        let mut connection = connection();
        connection.set_cwd(Some("/home/me".to_string()));
        connection.set_terminal_shell(Some(TerminalShellType::Wsl));
        connection.set_terminal_shell_wsl_distro(Some("Debian".to_string()));
        tab.set_connection(Some(connection));
    });
    assert_eq!(tab_tooltip(&tab), "~ (/home/me · Debian)");
    assert_eq!(
        local_shell_title(Some("WSL"), Some(&shell_connection(TerminalShellType::Wsl))).as_deref(),
        Some("WSL")
    );
    assert_eq!(
        local_shell_title(
            Some("pwsh"),
            Some(&shell_connection(TerminalShellType::Pwsh))
        )
        .as_deref(),
        Some("pwsh")
    );
}

#[test]
fn shows_a_running_command_alone_while_no_other_tab_runs_the_same() {
    let first = local_tab("1", "/work", |tab| tab.set_title("tTerm".to_string()));
    let second = local_tab("2", "/work", |tab| tab.set_title("web".to_string()));
    let third = local_tab("3", "/work", |tab| tab.set_title("api".to_string()));
    let mut commands = IndexMap::new();
    commands.insert("1".to_string(), "claude".to_string());
    commands.insert("2".to_string(), "codex --yolo".to_string());
    assert_eq!(
        running_command_title(&first, &commands).as_deref(),
        Some("claude")
    );
    assert_eq!(
        running_command_title(&second, &commands).as_deref(),
        Some("codex --yolo")
    );
    assert_eq!(running_command_title(&third, &commands), None);
}

#[test]
fn adds_the_tabs_title_while_another_tab_runs_a_command_of_the_same_title() {
    let first = local_tab("1", "/work", |tab| tab.set_title("tTerm".to_string()));
    let second = local_tab("2", "/work", |tab| tab.set_title("web/src".to_string()));
    let third = local_tab("3", "/work", |tab| tab.set_title("api".to_string()));
    let mut commands = IndexMap::new();
    commands.insert("1".to_string(), "claude".to_string());
    commands.insert(
        "2".to_string(),
        "claude --dangerously-skip-permissions".to_string(),
    );
    commands.insert("3".to_string(), "codex".to_string());
    assert_eq!(
        running_command_title(&first, &commands).as_deref(),
        Some("claude · tTerm")
    );
    assert_eq!(
        running_command_title(&second, &commands).as_deref(),
        Some("claude · web/src")
    );
    assert_eq!(
        running_command_title(&third, &commands).as_deref(),
        Some("codex")
    );
}

#[test]
fn leaves_a_tab_the_user_named_to_its_name() {
    let named = local_tab("1", "/work", |tab| {
        tab.set_title("Agent".to_string());
        tab.set_auto_title(None);
    });
    let mut commands = IndexMap::new();
    commands.insert("1".to_string(), "claude".to_string());
    assert_eq!(running_command_title(&named, &commands), None);
}

#[test]
fn oracle_matches_unicode_directory_titles() {
    let oracle = oracle_11d2();
    for case in oracle["localTabTitle"]["directories"]
        .as_array()
        .expect("directory cases")
    {
        assert_eq!(
            serde_json::to_value(directory_title(
                case["cwd"].as_str().expect("cwd"),
                case["home"].as_str()
            ))
            .unwrap(),
            case["title"],
            "{case:?}"
        );
    }
}

#[test]
fn oracle_matches_utf16_command_titles_and_regex_edges() {
    let oracle = oracle_11d2();
    for case in oracle["localTabTitle"]["commands"]
        .as_array()
        .expect("command cases")
    {
        assert_eq!(
            serde_json::to_value(command_title(case["command"].as_str().expect("command")))
                .unwrap(),
            case["title"],
            "{case:?}"
        );
    }
    assert_eq!(MAX_COMMAND_TITLE_LENGTH, 20);
}

#[test]
fn oracle_matches_local_shell_titles_and_tooltips() {
    let oracle = oracle_11d2();
    for case in oracle["localTabTitle"]["localShellTitles"]
        .as_array()
        .expect("shell cases")
    {
        let connection: Option<TabConnection> = case
            .get("connection")
            .filter(|value| !value.is_null())
            .map(|value| serde_json::from_value(value.clone()).expect("a connection"));
        assert_eq!(
            serde_json::to_value(local_shell_title(
                case["shellName"].as_str(),
                connection.as_ref()
            ))
            .unwrap(),
            case["title"],
            "{case:?}"
        );
    }
    for case in oracle["localTabTitle"]["tooltips"]
        .as_array()
        .expect("tooltip cases")
    {
        let tab: Tab = serde_json::from_value(case["tab"].clone()).expect("a tab");
        assert_eq!(
            tab_tooltip(&tab),
            case["tooltip"].as_str().expect("tooltip")
        );
    }
}

#[test]
fn oracle_matches_with_local_cwd() {
    let oracle = oracle_11d2();
    for case in oracle["localTabTitle"]["cwds"]
        .as_array()
        .expect("cwd cases")
    {
        let tab: Tab = serde_json::from_value(case["tab"].clone()).expect("a tab");
        let next = with_local_cwd(&tab, case["cwd"].as_str().expect("cwd"), None);
        assert_eq!(
            serde_json::to_value(&next).unwrap(),
            case["next"],
            "{case:?}"
        );
        assert_eq!(
            next == tab,
            case["same"].as_bool().expect("same"),
            "{case:?}"
        );
    }
}

#[test]
fn oracle_matches_running_command_titles() {
    let oracle = oracle_11d2();
    let commands = IndexMap::from([
        ("1".to_string(), "claude".to_string()),
        (
            "2".to_string(),
            "claude --dangerously-skip-permissions".to_string(),
        ),
        ("3".to_string(), "codex".to_string()),
        ("4".to_string(), "claude".to_string()),
        ("5".to_string(), String::new()),
        ("6".to_string(), "echo 😀😀😀😀😀😀😀😀".to_string()),
        ("7".to_string(), "echo".to_string()),
    ]);
    for case in oracle["localTabTitle"]["running"]
        .as_array()
        .expect("running cases")
    {
        let tab: Tab = serde_json::from_value(case["tab"].clone()).expect("a tab");
        assert_eq!(
            serde_json::to_value(running_command_title(&tab, &commands)).unwrap(),
            case["title"],
            "{case:?}"
        );
    }
}

#[test]
fn oracle_matches_distinct_titles_and_identity() {
    let oracle = oracle_11d2();
    for case in oracle["localTabTitle"]["distinct"]
        .as_array()
        .expect("distinct cases")
    {
        let tabs: Vec<Tab> = case["tabs"]
            .as_array()
            .expect("tabs")
            .iter()
            .map(|tab| serde_json::from_value(tab.clone()).expect("a tab"))
            .collect();
        let next = distinct_local_titles(&tabs, case["home"].as_str());
        assert_eq!(
            serde_json::to_value(next.iter().map(|tab| &tab.title).collect::<Vec<_>>()).unwrap(),
            case["titles"],
            "{case:?}"
        );
        assert_eq!(
            next != tabs,
            case["changed"].as_bool().expect("changed"),
            "{case:?}"
        );
    }
    let with_distinct = oracle["localTabTitle"]["withDistinct"].clone();
    let tabs = vec![local_tab("1", "/a/src", |tab| {
        tab.set_title("src".to_string())
    })];
    assert_eq!(
        serde_json::to_value(
            with_distinct_local_titles(&tabs, None)
                .into_iter()
                .map(|tab| tab.title.clone())
                .collect::<Vec<_>>()
        )
        .unwrap(),
        with_distinct
    );
}

#[test]
fn oracle_matches_legacy_titles() {
    let oracle = oracle_11d2();
    for case in oracle["localTabTitle"]["legacyTitles"]
        .as_array()
        .expect("legacy cases")
    {
        let mut terminal = tab("1", case["title"].as_str().expect("title"), |_| {});
        terminal.set_tab_type(TabType::Terminal);
        let mut ssh = terminal.clone();
        ssh.set_tab_type(TabType::Ssh);
        assert_eq!(
            has_legacy_local_title(&terminal),
            case["terminal"].as_bool().expect("terminal"),
            "{case:?}"
        );
        assert_eq!(
            has_legacy_local_title(&ssh),
            case["ssh"].as_bool().expect("ssh"),
            "{case:?}"
        );
    }
}
