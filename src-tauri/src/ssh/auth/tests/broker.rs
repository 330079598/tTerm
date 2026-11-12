//! Authentication protocol tests using the production broker transport.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tterm_core::events::{AppEvent, EventSink, ssh::SshEvent};
use tterm_core::prompts::PromptBroker;
use tterm_platform::window::MainWindow;

#[derive(Default)]
pub(super) struct Window(AtomicUsize);
impl MainWindow for Window {
    fn save_before_update(&self) -> tterm_platform::PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn send_to_background(&self, _: bool) -> Result<(), String> {
        Ok(())
    }

    fn show_and_focus(&self) -> Result<(), String> {
        Ok(())
    }
    fn reveal_for_prompt(&self) -> Result<(), String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn set_attention(&self, _: bool) -> Result<(), String> {
        Ok(())
    }
}
pub(super) struct Ui(pub(super) tokio::sync::mpsc::UnboundedSender<AppEvent>);
impl EventSink for Ui {
    fn emit(&self, event: AppEvent) -> Result<(), String> {
        self.0.send(event).map_err(|_| "test UI closed".into())
    }
}

async fn run(
    script: Script,
    answers: Vec<Option<Vec<&str>>>,
    method: AuthMethod<'_>,
) -> (
    Result<AuthUsed, String>,
    Vec<tterm_core::events::ssh::AuthPromptPayload>,
    usize,
) {
    let mut harness = Harness::start(script, vec![]).await;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let broker = Arc::new(PromptBroker::new(Arc::new(Ui(tx))));
    let owner = broker.register_owner(None).unwrap();
    let window = Arc::new(Window::default());
    let prompter = AuthPrompter::Scoped {
        broker: broker.clone(),
        owner: owner.clone(),
        window: window.clone(),
        tab_id: "p1-05-broker".into(),
    };
    let answers: VecDeque<_> = answers
        .into_iter()
        .map(|a| a.map(|a| a.into_iter().map(String::from).collect::<Vec<_>>()))
        .collect();
    let ui = tokio::spawn(async move {
        let mut answers = answers;
        let mut asked = Vec::new();
        while let Some(event) = rx.recv().await {
            if let AppEvent::Ssh(SshEvent::AuthPrompt { payload, .. }) = event {
                let answer = answers.pop_front().expect("unexpected question");
                broker.reply_auth(&payload.request_id, answer).unwrap();
                asked.push(payload);
            } else {
                panic!("unexpected event: {event:?}");
            }
            if answers.is_empty() {
                break;
            }
        }
        asked
    });
    let result = harness.authenticate_with(method, prompter).await;
    (result, ui.await.unwrap(), window.0.load(Ordering::SeqCst))
}

#[tokio::test]
async fn broker_auto_wrong_password_retries_then_succeeds() {
    let script = Script {
        password: || Auth::Accept,
        methods: Some(vec![MethodKind::Password]),
        after_wrong_password: Some(vec![MethodKind::Password]),
        ..Default::default()
    };
    let (result, asked, revealed) = run(
        script,
        vec![Some(vec!["wrong"]), Some(vec![PASSWORD])],
        AuthMethod::Auto {
            key_path: None,
            local_identities: false,
            tab_id: None,
        },
    )
    .await;
    assert_eq!(result.unwrap(), AuthUsed::Password);
    assert_eq!(asked.len(), 2);
    assert_ne!(asked[0].request_id, asked[1].request_id);
    assert_eq!(
        asked[1].instructions,
        "Permission denied, please try again."
    );
    assert_eq!(revealed, 2);
}

#[tokio::test]
async fn broker_mfa_preserves_round_order_and_fields() {
    let script = Script {
        methods: Some(vec![MethodKind::KeyboardInteractive]),
        rounds: vec![password_round(), code_round()],
        ..Default::default()
    };
    let (result, asked, revealed) = run(
        script,
        vec![Some(vec![PASSWORD]), Some(vec!["123456"])],
        AuthMethod::KeyboardInteractive,
    )
    .await;
    assert_eq!(result.unwrap(), AuthUsed::KeyboardInteractive);
    assert_eq!(asked.len(), 2);
    assert_eq!(asked[0].prompts, vec![field("Password: ", false)]);
    assert_eq!(asked[1].prompts, vec![field("Verification code: ", false)]);
    assert_eq!(revealed, 2);
}

#[tokio::test]
async fn broker_user_cancel_stops_authentication() {
    let script = Script {
        methods: Some(vec![MethodKind::KeyboardInteractive]),
        rounds: vec![password_round(), code_round()],
        ..Default::default()
    };
    let (result, asked, revealed) = run(script, vec![None], AuthMethod::KeyboardInteractive).await;
    assert_eq!(result.unwrap_err(), "SSH authentication was cancelled");
    assert_eq!(asked.len(), 1);
    assert_eq!(revealed, 1);
}
