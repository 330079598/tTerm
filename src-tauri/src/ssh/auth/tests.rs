//! Drives [`authenticate`] against an in-process `russh` server over a
//! duplex stream, so every case speaks the real authentication protocol.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::server::{Auth, Response};
use russh::{MethodKind, MethodSet};

use super::{
    authenticate, is_password_prompt, AuthMethod, AuthPromptField, AuthPrompter, AuthTarget,
    ScriptedPrompts,
};

const USER: &str = "ops";
const PASSWORD: &str = "hunter2";

/// One keyboard-interactive round: the prompts sent and the answers required.
#[derive(Clone)]
struct Round {
    prompts: Vec<(&'static str, bool)>,
    expect: Vec<&'static str>,
}

#[derive(Clone)]
struct Script {
    /// Verdict of the password method for the right password.
    password: fn() -> Auth,
    /// Methods still offered after a rejected password; `None` keeps russh's
    /// default of everything but `password`.
    after_wrong_password: Option<Vec<MethodKind>>,
    /// Verdict for any public key.
    publickey: fn() -> Auth,
    rounds: Vec<Round>,
    /// Verdict once every round was answered correctly.
    keyboard_interactive: fn() -> Auth,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            password: Auth::reject,
            after_wrong_password: None,
            publickey: Auth::reject,
            rounds: Vec::new(),
            keyboard_interactive: || Auth::Accept,
        }
    }
}

fn reject_offering(methods: &[MethodKind], partial_success: bool) -> Auth {
    Auth::Reject {
        proceed_with_methods: Some(MethodSet::from(methods)),
        partial_success,
    }
}

struct Server {
    script: Script,
    round: usize,
    /// What the server was sent, in order.
    log: Arc<Mutex<Vec<String>>>,
}

impl Server {
    fn challenge(&self) -> Auth {
        match self.script.rounds.get(self.round) {
            Some(round) => Auth::Partial {
                name: Cow::Borrowed("Verification"),
                instructions: Cow::Borrowed("Answer the questions"),
                prompts: Cow::Owned(
                    round
                        .prompts
                        .iter()
                        .map(|(text, echo)| (Cow::Borrowed(*text), *echo))
                        .collect(),
                ),
            },
            None => (self.script.keyboard_interactive)(),
        }
    }
}

impl russh::server::Handler for Server {
    type Error = russh::Error;

    async fn auth_password(&mut self, _user: &str, password: &str) -> Result<Auth, Self::Error> {
        self.log
            .lock()
            .unwrap()
            .push(format!("password:{password}"));
        if password == PASSWORD {
            return Ok((self.script.password)());
        }
        Ok(match &self.script.after_wrong_password {
            Some(methods) => reject_offering(methods, false),
            None => Auth::reject(),
        })
    }

    async fn auth_publickey(
        &mut self,
        _user: &str,
        _public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        self.log.lock().unwrap().push("publickey".to_string());
        Ok((self.script.publickey)())
    }

    async fn auth_keyboard_interactive<'a>(
        &'a mut self,
        _user: &str,
        _submethods: &str,
        response: Option<Response<'a>>,
    ) -> Result<Auth, Self::Error> {
        let Some(response) = response else {
            self.round = 0;
            self.log.lock().unwrap().push("kbd:start".to_string());
            return Ok(self.challenge());
        };

        let answers = response
            .map(|answer| String::from_utf8_lossy(&answer).into_owned())
            .collect::<Vec<_>>();
        self.log
            .lock()
            .unwrap()
            .push(format!("kbd:{}", answers.join("|")));
        let expected = &self.script.rounds[self.round].expect;
        if answers
            .iter()
            .map(String::as_str)
            .ne(expected.iter().copied())
        {
            return Ok(Auth::reject());
        }
        self.round += 1;
        Ok(self.challenge())
    }
}

struct Client;

impl russh::client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

struct Harness {
    handle: russh::client::Handle<Client>,
    log: Arc<Mutex<Vec<String>>>,
    prompts: Arc<Mutex<ScriptedPrompts>>,
}

impl Harness {
    async fn start(script: Script, answers: Vec<Option<Vec<&str>>>) -> Self {
        let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
        let log = Arc::new(Mutex::new(Vec::new()));
        let server_config = Arc::new(russh::server::Config {
            keys: vec![russh::keys::PrivateKey::random(
                &mut rand::rng(),
                russh::keys::ssh_key::Algorithm::Ed25519,
            )
            .expect("host key")],
            // Rejections are delayed to a constant time; keep the tests fast.
            auth_rejection_time: Duration::from_millis(1),
            auth_rejection_time_initial: Some(Duration::from_millis(1)),
            ..Default::default()
        });
        let server = Server {
            script,
            round: 0,
            log: log.clone(),
        };
        tokio::spawn(
            async move { russh::server::run_stream(server_config, server_io, server).await },
        );

        let handle = russh::client::connect_stream(
            Arc::new(russh::client::Config::default()),
            client_io,
            Client,
        )
        .await
        .expect("connect client");

        let answers = answers
            .into_iter()
            .map(|answer| answer.map(|values| values.into_iter().map(str::to_string).collect()))
            .collect::<VecDeque<_>>();
        Self {
            handle,
            log,
            prompts: Arc::new(Mutex::new(ScriptedPrompts {
                answers,
                asked: Vec::new(),
            })),
        }
    }

    async fn authenticate(&mut self, method: AuthMethod<'_>) -> Result<(), String> {
        self.authenticate_with(method, AuthPrompter::Scripted(self.prompts.clone()))
            .await
    }

    async fn authenticate_with(
        &mut self,
        method: AuthMethod<'_>,
        prompter: AuthPrompter,
    ) -> Result<(), String> {
        let target = AuthTarget {
            host: "example.com",
            port: 22,
            username: USER,
            hop: None,
        };
        authenticate(&mut self.handle, &target, method, &prompter)
            .await
            .map_err(|error| {
                assert!(!error.is_retryable(), "auth failures must not be retried");
                error.to_string()
            })
    }

    fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    /// The prompt texts of every question put to the user.
    fn asked(&self) -> Vec<Vec<AuthPromptField>> {
        self.prompts
            .lock()
            .unwrap()
            .asked
            .iter()
            .map(|payload| payload.prompts.clone())
            .collect()
    }
}

fn field(text: &str, echo: bool) -> AuthPromptField {
    AuthPromptField {
        text: text.to_string(),
        echo,
    }
}

fn password_round() -> Round {
    Round {
        prompts: vec![("Password: ", false)],
        expect: vec![PASSWORD],
    }
}

fn code_round() -> Round {
    Round {
        prompts: vec![("Verification code: ", false)],
        expect: vec!["123456"],
    }
}

#[tokio::test]
async fn password_method_succeeds_without_prompting() {
    let mut harness = Harness::start(
        Script {
            password: || Auth::Accept,
            ..Script::default()
        },
        vec![],
    )
    .await;

    harness
        .authenticate(AuthMethod::Password(Some(PASSWORD)))
        .await
        .expect("authenticate");
    assert_eq!(harness.log(), [format!("password:{PASSWORD}")]);
    assert!(harness.asked().is_empty());
}

#[tokio::test]
async fn wrong_password_fails_without_falling_back_while_password_is_still_offered() {
    let mut harness = Harness::start(
        Script {
            password: || Auth::Accept,
            after_wrong_password: Some(vec![MethodKind::Password, MethodKind::KeyboardInteractive]),
            rounds: vec![password_round()],
            ..Script::default()
        },
        vec![],
    )
    .await;

    let error = harness
        .authenticate(AuthMethod::Password(Some("wrong")))
        .await
        .expect_err("wrong password");
    assert_eq!(error, "SSH authentication failed");
    // One attempt only: no second guess through keyboard-interactive.
    assert_eq!(harness.log(), ["password:wrong"]);
}

#[tokio::test]
async fn stored_password_answers_a_pam_password_prompt() {
    // `PasswordAuthentication no`: only keyboard-interactive is offered.
    let mut harness = Harness::start(
        Script {
            password: Auth::reject,
            rounds: vec![password_round()],
            ..Script::default()
        },
        vec![],
    )
    .await;

    harness
        .authenticate(AuthMethod::Password(Some(PASSWORD)))
        .await
        .expect("authenticate");
    assert_eq!(
        harness.log(),
        [
            format!("password:{PASSWORD}"),
            "kbd:start".to_string(),
            format!("kbd:{PASSWORD}")
        ]
    );
    assert!(harness.asked().is_empty());
}

#[tokio::test]
async fn one_time_code_after_the_password_is_asked_from_the_user() {
    let mut harness = Harness::start(
        Script {
            password: Auth::reject,
            rounds: vec![password_round(), code_round()],
            ..Script::default()
        },
        vec![Some(vec!["123456"])],
    )
    .await;

    harness
        .authenticate(AuthMethod::Password(Some(PASSWORD)))
        .await
        .expect("authenticate");
    assert_eq!(harness.asked(), [vec![field("Verification code: ", false)]]);
    let asked = harness.prompts.lock().unwrap().asked[0].clone();
    assert_eq!(asked.kind, "keyboard-interactive");
    assert_eq!(asked.name, "Verification");
    assert_eq!(asked.instructions, "Answer the questions");
    assert_eq!((asked.host.as_str(), asked.port), ("example.com", 22));
    assert_eq!(asked.username, USER);
}

#[tokio::test]
async fn the_stored_password_answers_only_one_prompt() {
    // A second password prompt means the first answer was not what the
    // server wanted, so it goes to the user instead of repeating the guess.
    let mut harness = Harness::start(
        Script {
            password: Auth::reject,
            rounds: vec![
                password_round(),
                Round {
                    prompts: vec![("Password: ", false)],
                    expect: vec!["typed"],
                },
            ],
            ..Script::default()
        },
        vec![Some(vec!["typed"])],
    )
    .await;

    harness
        .authenticate(AuthMethod::Password(Some(PASSWORD)))
        .await
        .expect("authenticate");
    assert_eq!(harness.asked(), [vec![field("Password: ", false)]]);
    assert_eq!(
        harness.log(),
        [
            format!("password:{PASSWORD}"),
            "kbd:start".to_string(),
            format!("kbd:{PASSWORD}"),
            "kbd:typed".to_string()
        ]
    );
}

#[tokio::test]
async fn interactive_method_sends_every_prompt_to_the_user() {
    let mut harness = Harness::start(
        Script {
            rounds: vec![
                Round {
                    prompts: vec![("Username: ", true), ("Password: ", false)],
                    expect: vec!["alice", "pw"],
                },
                code_round(),
            ],
            ..Script::default()
        },
        vec![Some(vec!["alice", "pw"]), Some(vec!["123456"])],
    )
    .await;

    harness
        .authenticate(AuthMethod::KeyboardInteractive)
        .await
        .expect("authenticate");
    assert_eq!(
        harness.asked(),
        [
            vec![field("Username: ", true), field("Password: ", false)],
            vec![field("Verification code: ", false)],
        ]
    );
    assert_eq!(harness.log(), ["kbd:start", "kbd:alice|pw", "kbd:123456"]);
}

#[tokio::test]
async fn cancelling_a_prompt_fails_the_connection() {
    let mut harness = Harness::start(
        Script {
            rounds: vec![code_round()],
            ..Script::default()
        },
        vec![None],
    )
    .await;

    let error = harness
        .authenticate(AuthMethod::KeyboardInteractive)
        .await
        .expect_err("cancelled");
    assert_eq!(error, "SSH authentication was cancelled");
}

#[tokio::test]
async fn background_sessions_report_the_prompt_they_cannot_show() {
    let mut harness = Harness::start(
        Script {
            rounds: vec![code_round()],
            ..Script::default()
        },
        vec![],
    )
    .await;

    let error = harness
        .authenticate_with(AuthMethod::KeyboardInteractive, AuthPrompter::Unavailable)
        .await
        .expect_err("no prompt available");
    assert!(
        error.contains("needs input from you (Verification code)"),
        "{error}"
    );
}

#[tokio::test]
async fn a_wrong_code_is_rejected() {
    let mut harness = Harness::start(
        Script {
            rounds: vec![code_round()],
            ..Script::default()
        },
        vec![Some(vec!["000000"])],
    )
    .await;

    let error = harness
        .authenticate(AuthMethod::KeyboardInteractive)
        .await
        .expect_err("wrong code");
    assert_eq!(error, "SSH authentication failed");
}

/// Writes a fresh Ed25519 key to a temp file and returns its path.
fn write_client_key() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("tterm-auth-key-{}", uuid::Uuid::new_v4()));
    let key =
        russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::ssh_key::Algorithm::Ed25519)
            .expect("client key");
    std::fs::write(
        &path,
        key.to_openssh(russh::keys::ssh_key::LineEnding::LF)
            .expect("encode key")
            .as_bytes(),
    )
    .expect("write key");
    path
}

#[tokio::test]
async fn key_file_method_signs_with_the_key() {
    let mut harness = Harness::start(
        Script {
            publickey: || Auth::Accept,
            ..Script::default()
        },
        vec![],
    )
    .await;

    let key_path = write_client_key();
    let result = harness
        .authenticate(AuthMethod::Key {
            path: key_path.to_str().expect("utf-8 path"),
            passphrase: None,
        })
        .await;
    std::fs::remove_file(&key_path).ok();

    result.expect("authenticate");
    assert_eq!(harness.log(), ["publickey"]);
}

#[tokio::test]
async fn an_unreadable_key_is_reported_with_its_path() {
    let mut harness = Harness::start(Script::default(), vec![]).await;

    let error = harness
        .authenticate(AuthMethod::Key {
            path: "/nonexistent/tterm/id_ed25519",
            passphrase: None,
        })
        .await
        .expect_err("missing key");
    assert_eq!(
        error,
        "SSH key file not found: /nonexistent/tterm/id_ed25519"
    );
}

#[tokio::test]
async fn keyboard_interactive_then_password_asks_for_the_missing_password() {
    // `AuthenticationMethods keyboard-interactive,password` on a profile
    // that stores nothing.
    let mut harness = Harness::start(
        Script {
            password: || Auth::Accept,
            rounds: vec![code_round()],
            keyboard_interactive: || reject_offering(&[MethodKind::Password], true),
            ..Script::default()
        },
        vec![Some(vec!["123456"]), Some(vec![PASSWORD])],
    )
    .await;

    harness
        .authenticate(AuthMethod::KeyboardInteractive)
        .await
        .expect("authenticate");
    assert_eq!(
        harness.asked(),
        [
            vec![field("Verification code: ", false)],
            vec![field("Password:", false)]
        ]
    );
    assert_eq!(harness.prompts.lock().unwrap().asked[1].kind, "password");
}

#[tokio::test]
async fn a_missing_password_is_reported_before_contacting_the_server() {
    let mut harness = Harness::start(Script::default(), vec![]).await;

    let error = harness
        .authenticate(AuthMethod::Password(None))
        .await
        .expect_err("no password");
    assert_eq!(error, "SSH password is required");
    assert!(harness.log().is_empty());
}

#[test]
fn password_prompts_are_told_apart_from_code_prompts() {
    for prompt in [
        "Password: ",
        "(ops@host) Password:",
        "密码：",
        "ops 的口令:",
    ] {
        assert!(is_password_prompt(prompt), "{prompt}");
    }
    for prompt in [
        "Verification code: ",
        "One-time password (OATH) for `ops': ",
        "OTP Password:",
        "Please enter MFA code:",
        "请输入动态密码：",
        "Token:",
    ] {
        assert!(!is_password_prompt(prompt), "{prompt}");
    }
}

#[test]
fn plan_fields_pick_the_method_in_precedence_order() {
    let method = |agent, interactive, key, password| {
        AuthMethod::from_plan(agent, interactive, key, Some("phrase"), password)
    };
    assert!(matches!(
        method(true, true, Some("/k"), Some("p")),
        AuthMethod::Agent
    ));
    assert!(matches!(
        method(false, true, Some("/k"), Some("p")),
        AuthMethod::KeyboardInteractive
    ));
    assert!(matches!(
        method(false, false, Some("/k"), Some("p")),
        AuthMethod::Key {
            path: "/k",
            passphrase: Some("phrase")
        }
    ));
    assert!(matches!(
        method(false, false, None, Some("p")),
        AuthMethod::Password(Some("p"))
    ));
}
