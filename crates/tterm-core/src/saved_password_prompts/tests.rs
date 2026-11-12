use super::{
    FillRequest, PromptConnection, PromptConnectionKind, SavedPasswordPromptAction,
    SavedPasswordPromptState, SavedPasswordPrompts, SudoPasswordSource,
};

const SUDO_PROMPT: &str = "[sudo] password for alice:";
const NO_USER_PROMPT: &str = "[sudo: authenticate] Password:";

#[derive(Clone, Debug, PartialEq, Eq)]
struct Lookup {
    check_id: u64,
    prompt: String,
    user: Option<String>,
    profile_id: Option<String>,
    profile_name: Option<String>,
}

struct Harness {
    prompts: SavedPasswordPrompts,
    shown: Vec<Option<SavedPasswordPromptState>>,
    offers: Vec<Option<String>>,
    tab_prompt: Vec<bool>,
    sensitive: usize,
    lookups: Vec<Lookup>,
    fills: Vec<SavedPasswordPromptAction>,
    notices: usize,
}

impl Harness {
    fn new() -> Self {
        Self::with_connection(
            Some(ssh_connection("alice", Some("p1"), Some("web"))),
            false,
        )
    }

    fn with_connection(connection: Option<PromptConnection>, has_input_callback: bool) -> Self {
        Self {
            prompts: SavedPasswordPrompts::new(
                "tab".to_string(),
                1,
                connection,
                has_input_callback,
            ),
            shown: Vec::new(),
            offers: Vec::new(),
            tab_prompt: Vec::new(),
            sensitive: 0,
            lookups: Vec::new(),
            fills: Vec::new(),
            notices: 0,
        }
    }

    fn check(&mut self, line: &str, now_ms: u64) -> Vec<SavedPasswordPromptAction> {
        let actions = self.prompts.check_password_prompt(line, &[], now_ms);
        self.apply(&actions);
        actions
    }

    fn check_with_patterns(
        &mut self,
        line: &str,
        patterns: &[regress::Regex],
        now_ms: u64,
    ) -> Vec<SavedPasswordPromptAction> {
        let actions = self.prompts.check_password_prompt(line, patterns, now_ms);
        self.apply(&actions);
        actions
    }

    fn resolve(&mut self, source: Option<SudoPasswordSource>) -> Vec<SavedPasswordPromptAction> {
        let check_id = self
            .lookups
            .last()
            .expect("the prompt started a lookup")
            .check_id;
        let actions = self.prompts.resolve_saved_password_source(check_id, source);
        self.apply(&actions);
        actions
    }

    fn fail_lookup(&mut self) -> Vec<SavedPasswordPromptAction> {
        let check_id = self
            .lookups
            .last()
            .expect("the prompt started a lookup")
            .check_id;
        let actions = self.prompts.fail_saved_password_source(check_id);
        self.apply(&actions);
        actions
    }

    fn type_at_prompt(&mut self, data: &str) -> Vec<SavedPasswordPromptAction> {
        let actions = self.prompts.type_at_prompt(data);
        self.apply(&actions);
        actions
    }

    fn dismiss(&mut self) -> Vec<SavedPasswordPromptAction> {
        let actions = self.prompts.dismiss();
        self.apply(&actions);
        actions
    }

    fn fill(&mut self) -> FillRequest {
        let request = self.prompts.fill();
        self.apply(&request.actions);
        request
    }

    fn apply(&mut self, actions: &[SavedPasswordPromptAction]) {
        for action in actions {
            match action {
                SavedPasswordPromptAction::Show(state) => self.shown.push(state.clone()),
                SavedPasswordPromptAction::TabPrompt(prompt) => {
                    self.tab_prompt.push(*prompt);
                }
                SavedPasswordPromptAction::OfferChanged(prompt) => {
                    self.offers.push(prompt.clone());
                }
                SavedPasswordPromptAction::SensitivePrompt => self.sensitive += 1,
                SavedPasswordPromptAction::LookupSource {
                    check_id,
                    prompt,
                    user,
                    profile_id,
                    profile_name,
                } => self.lookups.push(Lookup {
                    check_id: *check_id,
                    prompt: prompt.clone(),
                    user: user.clone(),
                    profile_id: profile_id.clone(),
                    profile_name: profile_name.clone(),
                }),
                SavedPasswordPromptAction::FillThroughInput { .. }
                | SavedPasswordPromptAction::FillThroughCommand { .. } => {
                    self.fills.push(action.clone());
                }
                SavedPasswordPromptAction::NotifyNotSent => self.notices += 1,
            }
        }
    }
}

fn ssh_connection(
    username: &str,
    profile_id: Option<&str>,
    profile_name: Option<&str>,
) -> PromptConnection {
    PromptConnection {
        kind: PromptConnectionKind::Ssh,
        username: Some(username.to_string()),
        profile_id: profile_id.map(str::to_string),
        profile_name: profile_name.map(str::to_string),
        login_script: None,
    }
}

fn available(
    prompt: &str,
    user: Option<&str>,
    source: SudoPasswordSource,
) -> SavedPasswordPromptState {
    SavedPasswordPromptState::Available {
        prompt: prompt.to_string(),
        user: user.map(str::to_string),
        source,
    }
}

fn manual(prompt: &str, user: Option<&str>, typed: i64) -> SavedPasswordPromptState {
    SavedPasswordPromptState::Manual {
        prompt: prompt.to_string(),
        user: user.map(str::to_string),
        typed,
    }
}

#[test]
fn oracle_saved_source_offers_the_prompt_after_lookup() {
    let mut harness = Harness::new();
    let actions = harness.check(SUDO_PROMPT, 100_000);
    assert_eq!(
        actions,
        vec![
            SavedPasswordPromptAction::TabPrompt(true),
            SavedPasswordPromptAction::Show(None),
            SavedPasswordPromptAction::LookupSource {
                check_id: 2,
                prompt: SUDO_PROMPT.to_string(),
                user: Some("alice".to_string()),
                profile_id: Some("p1".to_string()),
                profile_name: Some("web".to_string()),
            },
        ]
    );
    assert_eq!(
        harness.prompts.current_prompt_key(),
        Some("0:[sudo] password for alice:")
    );
    assert!(harness.prompts.command_capture_suspended());
    assert_eq!(
        harness.prompts.command_capture_state(),
        &tterm_terminal::terminal_command_capture::empty_command_capture_state()
    );
    harness.resolve(Some(SudoPasswordSource::Login));
    assert_eq!(
        harness.shown,
        vec![
            None,
            Some(available(
                SUDO_PROMPT,
                Some("alice"),
                SudoPasswordSource::Login
            )),
        ]
    );
    assert_eq!(harness.offers, vec![Some(SUDO_PROMPT.to_string())]);
    assert_eq!(harness.sensitive, 0);
}

#[test]
fn oracle_missing_source_types_the_prompt_manually() {
    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    let actions = harness.resolve(None);
    assert_eq!(
        actions,
        vec![
            SavedPasswordPromptAction::SensitivePrompt,
            SavedPasswordPromptAction::Show(Some(manual(SUDO_PROMPT, Some("alice"), 0))),
        ]
    );
    assert_eq!(
        harness.shown,
        vec![None, Some(manual(SUDO_PROMPT, Some("alice"), 0))]
    );
    assert_eq!(harness.sensitive, 1);
    assert!(harness.offers.is_empty());
}

#[test]
fn oracle_empty_source_takes_the_manual_password_path() {
    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    let check_id = harness
        .lookups
        .last()
        .expect("the prompt started a lookup")
        .check_id;
    let actions = harness
        .prompts
        .resolve_saved_password_source_wire(check_id, Some(""));
    harness.apply(&actions);
    assert_eq!(
        actions,
        vec![
            SavedPasswordPromptAction::SensitivePrompt,
            SavedPasswordPromptAction::Show(Some(manual(SUDO_PROMPT, Some("alice"), 0))),
        ]
    );
    assert_eq!(
        harness.shown,
        vec![None, Some(manual(SUDO_PROMPT, Some("alice"), 0))]
    );
    assert_eq!(harness.sensitive, 1);
    assert!(harness.offers.is_empty());
    assert_eq!(SudoPasswordSource::from_wire(""), None);
    assert_eq!(
        SudoPasswordSource::from_wire("login"),
        Some(SudoPasswordSource::Login)
    );
    assert_eq!(
        SudoPasswordSource::from_wire("sudo"),
        Some(SudoPasswordSource::Sudo)
    );
    assert_eq!(SudoPasswordSource::from_wire("unknown"), None);
}

#[test]
fn oracle_another_user_never_receives_the_profiles_password() {
    let mut harness = Harness::new();
    let actions = harness.check("[sudo] password for root:", 100_000);
    assert_eq!(
        actions,
        vec![
            SavedPasswordPromptAction::TabPrompt(true),
            SavedPasswordPromptAction::Show(None),
            SavedPasswordPromptAction::SensitivePrompt,
            SavedPasswordPromptAction::Show(Some(manual(
                "[sudo] password for root:",
                Some("root"),
                0,
            ))),
        ]
    );
    assert!(harness.lookups.is_empty());
    assert_eq!(harness.sensitive, 1);
}

#[test]
fn oracle_empty_profile_id_and_name_type_manually() {
    let mut harness =
        Harness::with_connection(Some(ssh_connection("alice", Some(""), Some(""))), false);
    harness.check(SUDO_PROMPT, 100_000);
    assert!(harness.lookups.is_empty());
    assert_eq!(
        harness.shown.last(),
        Some(&Some(manual(SUDO_PROMPT, Some("alice"), 0)))
    );
}

#[test]
fn oracle_a_profile_name_without_an_id_still_looks_up_the_password() {
    let mut harness =
        Harness::with_connection(Some(ssh_connection("alice", Some(""), Some("web"))), false);
    harness.check(SUDO_PROMPT, 100_000);
    assert_eq!(
        harness.lookups.last(),
        Some(&Lookup {
            check_id: 2,
            prompt: SUDO_PROMPT.to_string(),
            user: Some("alice".to_string()),
            profile_id: Some(String::new()),
            profile_name: Some("web".to_string()),
        })
    );
    harness.resolve(Some(SudoPasswordSource::Sudo));
    assert_eq!(
        harness.shown.last(),
        Some(&Some(available(
            SUDO_PROMPT,
            Some("alice"),
            SudoPasswordSource::Sudo,
        )))
    );
}

#[test]
fn oracle_a_prompt_without_a_username_is_eligible_for_lookup() {
    let mut harness = Harness::new();
    harness.check(NO_USER_PROMPT, 100_000);
    assert_eq!(
        harness.lookups.last().map(|lookup| lookup.user.as_deref()),
        Some(None)
    );
    harness.resolve(Some(SudoPasswordSource::Login));
    assert_eq!(
        harness.shown.last(),
        Some(&Some(available(
            NO_USER_PROMPT,
            None,
            SudoPasswordSource::Login,
        )))
    );
}

#[test]
fn oracle_an_empty_username_still_allows_a_prompt_without_a_username() {
    let mut harness =
        Harness::with_connection(Some(ssh_connection("", Some("p1"), Some("web"))), false);
    harness.check(NO_USER_PROMPT, 100_000);
    assert!(harness.lookups.last().is_some());
    harness.resolve(Some(SudoPasswordSource::Login));
    assert_eq!(
        harness.shown.last(),
        Some(&Some(available(
            NO_USER_PROMPT,
            None,
            SudoPasswordSource::Login,
        )))
    );
}

#[test]
fn oracle_custom_prompt_users_follow_ts_empty_and_missing_username_rules() {
    let patterns =
        tterm_terminal::sudo_prompt::compile_prompt_patterns(&["^PIN:(?<user>.*)$".to_string()]);

    let mut missing_username = Harness::with_connection(
        Some(PromptConnection {
            username: None,
            ..ssh_connection("alice", Some("p1"), Some("web"))
        }),
        false,
    );
    missing_username.check_with_patterns("PIN:", &patterns, 100_000);
    assert!(missing_username.lookups.is_empty());
    assert_eq!(missing_username.sensitive, 1);
    assert_eq!(
        missing_username.shown.last(),
        Some(&Some(manual("PIN:", Some(""), 0)))
    );

    let mut empty_username =
        Harness::with_connection(Some(ssh_connection("", Some("p1"), Some("web"))), false);
    empty_username.check_with_patterns("PIN:", &patterns, 100_000);
    assert_eq!(
        empty_username.lookups,
        vec![Lookup {
            check_id: 2,
            prompt: "PIN:".to_string(),
            user: Some(String::new()),
            profile_id: Some("p1".to_string()),
            profile_name: Some("web".to_string()),
        }]
    );

    let mut named_prompt = Harness::with_connection(
        Some(PromptConnection {
            username: None,
            ..ssh_connection("alice", Some("p1"), Some("web"))
        }),
        false,
    );
    named_prompt.check(SUDO_PROMPT, 100_000);
    assert!(named_prompt.lookups.is_empty());
    assert_eq!(named_prompt.sensitive, 1);
    assert_eq!(
        named_prompt.shown.last(),
        Some(&Some(manual(SUDO_PROMPT, Some("alice"), 0)))
    );
}

#[test]
fn oracle_a_rejected_password_is_not_offered_again_in_the_session() {
    let mut harness = Harness::new();
    harness
        .prompts
        .note_password_written(true, 80_001, SUDO_PROMPT);
    harness.check(SUDO_PROMPT, 100_000);
    assert_eq!(
        harness.shown.last(),
        Some(&Some(SavedPasswordPromptState::Rejected {
            prompt: SUDO_PROMPT.to_string(),
            user: Some("alice".to_string()),
        }))
    );
    assert!(harness.lookups.is_empty());

    harness.prompts.note_line_feed();
    harness.check(SUDO_PROMPT, 100_001);
    assert!(harness.lookups.is_empty());
    assert_eq!(harness.sensitive, 2);
    assert_eq!(
        harness.shown.last(),
        Some(&Some(manual(SUDO_PROMPT, Some("alice"), 0)))
    );
    assert_eq!(
        harness.prompts.current_prompt_key(),
        Some("1:[sudo] password for alice:")
    );
}

#[test]
fn oracle_a_fill_refused_one_millisecond_before_the_window_is_rejected() {
    let mut harness = Harness::new();
    harness
        .prompts
        .note_password_written(true, 80_001, SUDO_PROMPT);
    let actions = harness.check(SUDO_PROMPT, 100_000);
    assert_eq!(
        actions,
        vec![
            SavedPasswordPromptAction::TabPrompt(true),
            SavedPasswordPromptAction::Show(None),
            SavedPasswordPromptAction::Show(Some(SavedPasswordPromptState::Rejected {
                prompt: SUDO_PROMPT.to_string(),
                user: Some("alice".to_string()),
            })),
            SavedPasswordPromptAction::SensitivePrompt,
        ]
    );
    assert_eq!(
        harness.shown,
        vec![
            None,
            Some(SavedPasswordPromptState::Rejected {
                prompt: SUDO_PROMPT.to_string(),
                user: Some("alice".to_string()),
            }),
        ]
    );
    assert!(harness.lookups.is_empty());
    assert_eq!(harness.sensitive, 1);
}

#[test]
fn oracle_a_fill_exactly_at_the_window_is_looked_up_again() {
    let mut harness = Harness::new();
    harness
        .prompts
        .note_password_written(true, 80_000, SUDO_PROMPT);
    harness.check(SUDO_PROMPT, 100_000);
    assert_eq!(harness.lookups.len(), 1);
    harness.resolve(Some(SudoPasswordSource::Login));
    assert_eq!(
        harness.shown,
        vec![
            None,
            Some(available(
                SUDO_PROMPT,
                Some("alice"),
                SudoPasswordSource::Login
            )),
        ]
    );
    assert_eq!(
        harness.shown.last(),
        Some(&Some(available(
            SUDO_PROMPT,
            Some("alice"),
            SudoPasswordSource::Login,
        )))
    );
}

#[test]
fn oracle_a_different_prompt_after_a_fill_is_looked_up_again() {
    let mut harness = Harness::new();
    harness
        .prompts
        .note_password_written(true, 100_000, "other");
    harness.check(SUDO_PROMPT, 100_000);
    assert_eq!(harness.lookups.len(), 1);
    harness.resolve(Some(SudoPasswordSource::Login));
    assert_eq!(
        harness.shown,
        vec![
            None,
            Some(available(
                SUDO_PROMPT,
                Some("alice"),
                SudoPasswordSource::Login
            )),
        ]
    );
    assert_eq!(
        harness.shown.last(),
        Some(&Some(available(
            SUDO_PROMPT,
            Some("alice"),
            SudoPasswordSource::Login,
        )))
    );
}

#[test]
fn oracle_a_stale_lookup_cannot_revive_an_offer() {
    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    let check_id = harness.lookups[0].check_id;
    harness.check("$ ", 100_000);
    let actions = harness
        .prompts
        .resolve_saved_password_source(check_id, Some(SudoPasswordSource::Login));
    assert!(actions.is_empty());
    assert_eq!(harness.shown, vec![None, None]);
    assert!(harness.offers.is_empty());
}

#[test]
fn oracle_fill_sends_the_backend_command_once_and_records_a_written_fill() {
    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    harness.resolve(Some(SudoPasswordSource::Login));
    let first = harness.fill();
    assert_eq!(
        first,
        FillRequest {
            filled: true,
            prompt: Some(SUDO_PROMPT.to_string()),
            actions: vec![
                SavedPasswordPromptAction::OfferChanged(None),
                SavedPasswordPromptAction::Show(None),
                SavedPasswordPromptAction::FillThroughCommand {
                    tab_id: "tab".to_string(),
                    session_nonce: 1,
                    profile_id: Some("p1".to_string()),
                    profile_name: Some("web".to_string()),
                    prompt: SUDO_PROMPT.to_string(),
                },
            ],
        }
    );
    let second = harness.fill();
    assert_eq!(
        second,
        FillRequest {
            filled: false,
            prompt: None,
            actions: Vec::new(),
        }
    );
    harness
        .prompts
        .note_password_written(true, 50_000, SUDO_PROMPT);
    assert_eq!(
        harness.prompts.last_password_fill(),
        Some((SUDO_PROMPT, 50_000))
    );
    assert_eq!(harness.notices, 0);
    assert_eq!(
        harness.shown,
        vec![
            None,
            Some(available(
                SUDO_PROMPT,
                Some("alice"),
                SudoPasswordSource::Login
            )),
            None,
        ]
    );
}

#[test]
fn oracle_input_callback_receives_the_prompt_as_saved_password_data() {
    let mut harness = Harness::new();
    harness.prompts.set_has_input_callback(true);
    harness.check(SUDO_PROMPT, 100_000);
    harness.resolve(Some(SudoPasswordSource::Login));
    let request = harness.fill();
    assert_eq!(
        request.actions,
        vec![
            SavedPasswordPromptAction::OfferChanged(None),
            SavedPasswordPromptAction::Show(None),
            SavedPasswordPromptAction::FillThroughInput {
                tab_id: "tab".to_string(),
                session_nonce: 1,
                data: SUDO_PROMPT.to_string(),
                kind: "saved-password".to_string(),
            },
        ]
    );
    assert_eq!(
        harness.shown,
        vec![
            None,
            Some(available(
                SUDO_PROMPT,
                Some("alice"),
                SudoPasswordSource::Login
            )),
            None,
        ]
    );
    harness
        .prompts
        .note_password_written(true, 50_000, SUDO_PROMPT);
    assert_eq!(
        harness.prompts.last_password_fill(),
        Some((SUDO_PROMPT, 50_000))
    );
    assert_eq!(harness.notices, 0);
}

#[test]
fn oracle_a_rejected_backend_write_is_not_recorded_and_notifies() {
    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    harness.resolve(Some(SudoPasswordSource::Login));
    harness.fill();
    let actions = harness
        .prompts
        .note_password_written(false, 50_000, SUDO_PROMPT);
    assert_eq!(actions, vec![SavedPasswordPromptAction::NotifyNotSent]);
    harness.apply(&actions);
    assert_eq!(harness.prompts.last_password_fill(), None);
    assert_eq!(harness.notices, 1);
}

#[test]
fn oracle_a_refused_input_callback_write_does_not_notify() {
    let mut harness = Harness::new();
    harness.prompts.set_has_input_callback(true);
    harness.check(SUDO_PROMPT, 100_000);
    harness.resolve(Some(SudoPasswordSource::Login));
    harness.fill();
    let actions = harness
        .prompts
        .note_password_written(false, 50_000, SUDO_PROMPT);
    assert!(actions.is_empty());
    assert_eq!(harness.prompts.last_password_fill(), None);
    assert_eq!(harness.notices, 0);
}

#[test]
fn typing_counts_characters_until_enter_clears_the_prompt() {
    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    harness.resolve(None);
    assert_eq!(
        harness.shown.last(),
        Some(&Some(manual(SUDO_PROMPT, Some("alice"), 0)))
    );
    harness.type_at_prompt("s");
    harness.type_at_prompt("ecret");
    assert_eq!(
        harness.shown.last(),
        Some(&Some(manual(SUDO_PROMPT, Some("alice"), 6)))
    );
    harness.type_at_prompt("\u{007f}");
    assert_eq!(
        harness.shown.last(),
        Some(&Some(manual(SUDO_PROMPT, Some("alice"), 5)))
    );
    harness.type_at_prompt("\r");
    assert_eq!(harness.shown.last(), Some(&None));
    assert_eq!(
        harness.prompts.current_prompt_key(),
        Some("0:[sudo] password for alice:")
    );
}

#[test]
fn typing_over_an_offer_turns_it_into_the_manual_count() {
    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    harness.resolve(Some(SudoPasswordSource::Login));
    harness.type_at_prompt("s");
    assert_eq!(
        harness.shown.last(),
        Some(&Some(manual(SUDO_PROMPT, Some("alice"), 1)))
    );
}

#[test]
fn oracle_typed_password_count_iterates_unicode_and_ignores_empty_input() {
    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    harness.resolve(None);
    harness.type_at_prompt("密😀e\u{0301}");
    assert_eq!(
        harness.shown.last(),
        Some(&Some(manual(SUDO_PROMPT, Some("alice"), 4)))
    );
    harness.type_at_prompt("");
    assert_eq!(
        harness.shown.last(),
        Some(&Some(manual(SUDO_PROMPT, Some("alice"), 4)))
    );
    harness.type_at_prompt("\u{007f}");
    assert_eq!(
        harness.shown.last(),
        Some(&Some(manual(SUDO_PROMPT, Some("alice"), 3)))
    );
}

#[test]
fn dismissing_an_offer_types_manually_from_zero() {
    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    harness.resolve(Some(SudoPasswordSource::Login));
    harness.dismiss();
    assert_eq!(
        harness.shown.last(),
        Some(&Some(manual(SUDO_PROMPT, Some("alice"), 0)))
    );
    harness.type_at_prompt("s");
    assert_eq!(
        harness.shown.last(),
        Some(&Some(manual(SUDO_PROMPT, Some("alice"), 1)))
    );
}

#[test]
fn the_tab_mark_stays_until_the_password_is_submitted() {
    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    assert_eq!(harness.tab_prompt, vec![true]);
    harness.type_at_prompt("s");
    assert_eq!(harness.tab_prompt, vec![true]);
    harness.type_at_prompt("\r");
    assert_eq!(harness.tab_prompt, vec![true, false]);
    assert!(harness.prompts.shown().is_none());
}

#[test]
fn typing_at_the_shell_only_clears_the_prompt_display() {
    let mut harness = Harness::new();
    assert!(harness.check("alice@web:~$ ", 100_000).is_empty());
    assert!(harness.shown.is_empty());
    harness.type_at_prompt("l");
    assert!(harness.shown.iter().all(Option::is_none));
}

#[test]
fn oracle_a_failed_lookup_types_manually() {
    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    let actions = harness.fail_lookup();
    assert_eq!(
        actions,
        vec![
            SavedPasswordPromptAction::SensitivePrompt,
            SavedPasswordPromptAction::Show(Some(manual(SUDO_PROMPT, Some("alice"), 0))),
        ]
    );
    assert_eq!(harness.sensitive, 1);
}

#[test]
fn oracle_non_ssh_disposed_and_unchanged_lines_do_nothing() {
    let mut harness = Harness::with_connection(None, false);
    assert_eq!(harness.check(SUDO_PROMPT, 100_000), vec![]);
    assert_eq!(harness.prompts.current_prompt_key(), None);

    let mut harness = Harness::with_connection(
        Some(PromptConnection {
            kind: PromptConnectionKind::Terminal,
            ..ssh_connection("alice", Some("p1"), Some("web"))
        }),
        false,
    );
    assert_eq!(harness.check(SUDO_PROMPT, 100_000), vec![]);
    assert_eq!(harness.prompts.current_prompt_key(), None);

    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    assert_eq!(harness.check(SUDO_PROMPT, 100_000), vec![]);
    assert_eq!(
        harness.prompts.current_prompt_key(),
        Some("0:[sudo] password for alice:")
    );
    harness.prompts.dispose();
    assert_eq!(harness.check("[sudo] password for root:", 100_001), vec![]);
    assert_eq!(
        harness.prompts.current_prompt_key(),
        Some("0:[sudo] password for alice:")
    );
}

#[test]
fn oracle_login_script_runs_only_for_ssh_or_terminal_connections_with_lines() {
    let ssh = Harness::with_connection(
        Some(PromptConnection {
            login_script: Some("cd /srv\r\n\r\necho ready\r\n".to_string()),
            ..ssh_connection("alice", Some("p1"), Some("web"))
        }),
        false,
    );
    assert_eq!(
        ssh.prompts.login_script_lines(),
        Some(vec!["cd /srv".to_string(), "echo ready".to_string()])
    );

    let terminal = Harness::with_connection(
        Some(PromptConnection {
            kind: PromptConnectionKind::Terminal,
            login_script: None,
            ..ssh_connection("alice", Some("p1"), Some("web"))
        }),
        false,
    );
    assert_eq!(terminal.prompts.login_script_lines(), None);

    let other = Harness::with_connection(
        Some(PromptConnection {
            kind: PromptConnectionKind::Other,
            login_script: Some("echo skipped".to_string()),
            ..ssh_connection("alice", Some("p1"), Some("web"))
        }),
        false,
    );
    assert_eq!(other.prompts.login_script_lines(), None);

    let empty = Harness::with_connection(
        Some(PromptConnection {
            login_script: Some(String::new()),
            ..ssh_connection("alice", Some("p1"), Some("web"))
        }),
        false,
    );
    assert_eq!(empty.prompts.login_script_lines(), None);

    let blank = Harness::with_connection(
        Some(PromptConnection {
            login_script: Some(" \u{3000}\r\n".to_string()),
            ..ssh_connection("alice", Some("p1"), Some("web"))
        }),
        false,
    );
    assert_eq!(blank.prompts.login_script_lines(), None);
}

#[test]
fn oracle_line_feeds_change_the_prompt_key_but_still_allow_one_offer() {
    let mut harness = Harness::new();
    for _ in 0..7 {
        harness.prompts.note_line_feed();
    }
    harness.check(SUDO_PROMPT, 100_000);
    assert_eq!(
        harness.prompts.current_prompt_key(),
        Some("7:[sudo] password for alice:")
    );
    harness.resolve(Some(SudoPasswordSource::Login));
    assert_eq!(
        harness.shown,
        vec![
            None,
            Some(available(
                SUDO_PROMPT,
                Some("alice"),
                SudoPasswordSource::Login
            )),
        ]
    );
}

#[test]
fn password_prompts_reset_command_capture_when_they_appear_and_when_filled() {
    let mut harness = Harness::new();
    harness.check(SUDO_PROMPT, 100_000);
    assert!(harness.prompts.command_capture_suspended());
    harness.resolve(Some(SudoPasswordSource::Login));
    assert!(harness.prompts.command_capture_suspended());
    harness.fill();
    assert!(!harness.prompts.command_capture_suspended());
    assert_eq!(
        harness.prompts.command_capture_state(),
        &tterm_terminal::terminal_command_capture::empty_command_capture_state()
    );
}

#[test]
fn sudo_password_source_json_matches_the_backend_enum() {
    assert_eq!(
        serde_json::to_value(SudoPasswordSource::Sudo).expect("the source serializes"),
        serde_json::json!("sudo")
    );
    assert_eq!(
        serde_json::to_value(SudoPasswordSource::Login).expect("the source serializes"),
        serde_json::json!("login")
    );
    let source: SudoPasswordSource =
        serde_json::from_value(serde_json::json!("login")).expect("the source deserializes");
    assert_eq!(source, SudoPasswordSource::Login);
}
