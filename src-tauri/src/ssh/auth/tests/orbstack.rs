//! Explicitly run against this card's throwaway containers, never real profiles.
use super::broker::{Ui, Window};
use super::*;
use russh::{ChannelMsg, Disconnect};
use tterm_core::events::{
    ssh::{SshEvent, SshHostKeyPromptPayload},
    AppEvent,
};
use tterm_core::prompts::PromptBroker;

struct Host {
    broker: Arc<PromptBroker>,
    owner: tterm_core::prompts::PromptOwner,
    label: String,
}
impl russh::client::Handler for Host {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = match key {
            russh::keys::PublicKeyOrCertificate::PublicKey { key, .. } => key,
            _ => return Ok(false),
        };
        let payload = SshHostKeyPromptPayload {
            request_id: uuid::Uuid::new_v4().to_string(),
            profile_name: self.label.clone(),
            host: self.label.clone(),
            port: 22,
            algorithm: key.algorithm().to_string(),
            fingerprint: key
                .fingerprint(russh::keys::ssh_key::HashAlg::Sha256)
                .to_string(),
            reason: "unknown".into(),
            known_fingerprint: None,
        };
        let wait =
            crate::prompt_scope::host_key(&self.broker, &self.owner, "p1-05-orbstack", payload)
                .unwrap();
        Ok(matches!(
            wait.wait().await.unwrap(),
            tterm_core::prompts::PromptOutcome::Answered(
                tterm_core::prompts::PromptReply::HostKey(true)
            )
        ))
    }
}

async fn auth(
    session: &mut russh::client::Handle<Host>,
    prompter: &AuthPrompter,
    host: &str,
    hop: Option<(usize, usize)>,
    method: AuthMethod<'_>,
) {
    authenticate(
        session,
        &AuthTarget {
            host,
            port: 22,
            username: "alice",
            hop,
        },
        method,
        prompter,
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires tterm-P1-05-hop:22506 and host:22505, run explicitly"]
async fn orbstack_two_hops_preserve_host_key_then_auth_order() {
    assert_eq!(std::env::var("TTERM_P1_05_ORBSTACK").as_deref(), Ok("1"));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let broker = Arc::new(PromptBroker::new(Arc::new(Ui(tx))));
    let owner = broker.register_owner(None).unwrap();
    let ui_broker = broker.clone();
    let ui = tokio::spawn(async move {
        let mut sequence = Vec::new();
        let mut first_password = true;
        while let Some(event) = rx.recv().await {
            match event {
                AppEvent::Ssh(SshEvent::HostKeyPrompt { payload, .. }) => {
                    sequence.push(format!("key:{}", payload.host));
                    ui_broker.reply_host_key(&payload.request_id, true).unwrap();
                }
                AppEvent::Ssh(SshEvent::AuthPrompt { payload, .. }) => {
                    sequence.push(format!("auth:{}:{}", payload.host, payload.kind));
                    let answer = if first_password {
                        first_password = false;
                        "wrong"
                    } else {
                        "alice-pass"
                    };
                    ui_broker
                        .reply_auth(&payload.request_id, Some(vec![answer.into()]))
                        .unwrap();
                    if sequence.len() == 7 {
                        break;
                    }
                }
                _ => panic!("unexpected event"),
            }
        }
        sequence
    });
    let make_handler = |label: &str| Host {
        broker: broker.clone(),
        owner: owner.clone(),
        label: label.into(),
    };
    let prompter = AuthPrompter::Scoped {
        broker: broker.clone(),
        owner: owner.clone(),
        tab_id: "p1-05-orbstack".into(),
        window: Arc::new(Window::default()),
    };
    let auto = || AuthMethod::Auto {
        key_path: None,
        local_identities: false,
        tab_id: None,
    };
    let mut hop1 = russh::client::connect(
        Arc::new(russh::client::Config::default()),
        ("127.0.0.1", 22506),
        make_handler("hop1"),
    )
    .await
    .unwrap();
    auth(&mut hop1, &prompter, "hop1", Some((1, 2)), auto()).await;
    let stream = hop1
        .channel_open_direct_tcpip("tterm-P1-05-host", 22, "127.0.0.1", 0)
        .await
        .unwrap()
        .into_stream();
    let mut hop2 = russh::client::connect_stream(
        Arc::new(russh::client::Config::default()),
        stream,
        make_handler("hop2"),
    )
    .await
    .unwrap();
    auth(
        &mut hop2,
        &prompter,
        "hop2",
        Some((2, 2)),
        AuthMethod::KeyboardInteractive,
    )
    .await;
    let stream = hop2
        .channel_open_direct_tcpip("127.0.0.1", 22, "127.0.0.1", 0)
        .await
        .unwrap()
        .into_stream();
    let mut target = russh::client::connect_stream(
        Arc::new(russh::client::Config::default()),
        stream,
        make_handler("target"),
    )
    .await
    .unwrap();
    auth(
        &mut target,
        &prompter,
        "target",
        None,
        AuthMethod::KeyboardInteractive,
    )
    .await;
    let mut channel = target.channel_open_session().await.unwrap();
    channel.exec(true, "printf p1-05-chain-ok").await.unwrap();
    let mut output = Vec::new();
    while let Some(message) = channel.wait().await {
        if let ChannelMsg::Data { data } = message {
            output.extend_from_slice(&data);
        }
    }
    assert_eq!(String::from_utf8(output).unwrap(), "p1-05-chain-ok");
    assert_eq!(
        ui.await.unwrap(),
        vec![
            "key:hop1",
            "auth:hop1:password",
            "auth:hop1:password",
            "key:hop2",
            "auth:hop2:keyboard-interactive",
            "key:target",
            "auth:target:keyboard-interactive"
        ]
    );
    target
        .disconnect(Disconnect::ByApplication, "done", "en")
        .await
        .unwrap();
    hop2.disconnect(Disconnect::ByApplication, "done", "en")
        .await
        .unwrap();
    hop1.disconnect(Disconnect::ByApplication, "done", "en")
        .await
        .unwrap();
    broker.cancel_owner(&owner);
}
