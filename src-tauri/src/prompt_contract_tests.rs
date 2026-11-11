//! P1-05 baselines captured before prompt payloads or transports are migrated.
use tterm_core::events::proxy::ProxyCommandPrompt;
use tterm_core::events::ssh::AuthPromptField;
use tterm_core::events::ssh::AuthPromptPayload;
use tterm_core::events::ssh::SshHostKeyPromptPayload;

use serde_json::{json, Value};

fn host_key(profile: &str, known: Option<&str>) -> SshHostKeyPromptPayload {
    SshHostKeyPromptPayload {
        request_id: "host-request".into(),
        profile_name: profile.into(),
        host: "test.invalid".into(),
        port: 22505,
        algorithm: "ssh-ed25519".into(),
        fingerprint: "SHA256:new".into(),
        reason: if known.is_some() {
            "mismatch"
        } else {
            "unknown"
        }
        .into(),
        known_fingerprint: known.map(String::from),
    }
}

#[test]
fn prompt_contract_host_key_unknown_and_changed_include_null_or_old_fingerprint() {
    let expected = json!({
        "requestId": "host-request", "profileName": "p1-05", "host": "test.invalid",
        "port": 22505, "algorithm": "ssh-ed25519", "fingerprint": "SHA256:new",
        "reason": "unknown", "knownFingerprint": null
    });
    assert_eq!(host_wire(host_key("p1-05", None)), expected);
    let mut changed = expected;
    changed["reason"] = json!("mismatch");
    changed["knownFingerprint"] = json!("SHA256:old");
    assert_eq!(host_wire(host_key("p1-05", Some("SHA256:old"))), changed);
}

#[test]
fn prompt_contract_jump_host_uses_synthetic_profile_without_hop_fields() {
    let profile = format!(
        "{}test.invalid:22505",
        crate::ssh::store::JUMP_HOST_PROFILE_PREFIX
    );
    assert_eq!(
        host_wire(host_key(&profile, None)),
        json!({
            "requestId": "host-request", "profileName": "jump:test.invalid:22505",
            "host": "test.invalid", "port": 22505, "algorithm": "ssh-ed25519",
            "fingerprint": "SHA256:new", "reason": "unknown", "knownFingerprint": null
        })
    );
}

fn auth(kind: &'static str) -> AuthPromptPayload {
    AuthPromptPayload {
        request_id: "auth-request".into(),
        host: "test.invalid".into(),
        port: 22505,
        username: "alice".into(),
        hop_index: None,
        total_hops: None,
        kind,
        name: String::new(),
        instructions: String::new(),
        prompts: vec![AuthPromptField {
            text: "Password:".into(),
            echo: false,
        }],
        retry: false,
    }
}

#[test]
fn prompt_contract_auth_kinds_omit_optional_hop_fields() {
    for kind in ["password", "passphrase", "sudo", "keyboard-interactive"] {
        assert_eq!(
            auth_wire(auth(kind)),
            json!({
                "requestId": "auth-request", "host": "test.invalid", "port": 22505,
                "username": "alice", "kind": kind, "name": "", "instructions": "",
                "prompts": [{"text": "Password:", "echo": false}], "retry": false
            })
        );
    }
}

#[test]
fn prompt_contract_auth_multi_field_round_and_sudo_retry() {
    let mut payload = auth("keyboard-interactive");
    payload.hop_index = Some(2);
    payload.total_hops = Some(3);
    payload.name = "MFA".into();
    payload.instructions = "验证码".into();
    payload.prompts = vec![
        AuthPromptField {
            text: "Account:".into(),
            echo: true,
        },
        AuthPromptField {
            text: "OTP:".into(),
            echo: false,
        },
    ];
    assert_eq!(
        auth_wire(payload),
        json!({
            "requestId": "auth-request", "host": "test.invalid", "port": 22505,
            "username": "alice", "hopIndex": 2, "totalHops": 3,
            "kind": "keyboard-interactive", "name": "MFA", "instructions": "验证码",
            "prompts": [{"text": "Account:", "echo": true}, {"text": "OTP:", "echo": false}],
            "retry": false
        })
    );
    let mut sudo = auth("sudo");
    sudo.retry = true;
    sudo.prompts[0].text = "Password".into();
    assert_eq!(
        auth_wire(sudo),
        json!({
            "requestId": "auth-request", "host": "test.invalid", "port": 22505,
            "username": "alice", "kind": "sudo", "name": "", "instructions": "",
            "prompts": [{"text": "Password", "echo": false}], "retry": true
        })
    );
}

#[test]
fn prompt_contract_proxy_payload_and_expired_string() {
    let prompt = ProxyCommandPrompt {
        request_id: "legacy-command-hash".into(),
        proxy_name: "p1-05".into(),
        command: "nc %h %p".into(),
    };
    assert_eq!(
        wire(
            tterm_core::events::AppEvent::Proxy(
                tterm_core::events::proxy::ProxyEvent::CommandPrompt(prompt.clone())
            ),
            "proxy-command-prompt"
        ),
        json!({
            "requestId": "legacy-command-hash", "proxyName": "p1-05", "command": "nc %h %p"
        })
    );
    assert_eq!(
        serde_json::to_value(&prompt.request_id).unwrap(),
        json!("legacy-command-hash")
    );
    assert_eq!(
        serde_json::to_value("auth-request").unwrap(),
        Value::String("auth-request".into())
    );
}

fn wire(event: tterm_core::events::AppEvent, expected_name: &str) -> Value {
    let (name, target, payload) = crate::event_sink::TauriEventSink::wire(&event).unwrap();
    assert_eq!(name.as_string(), expected_name);
    assert_eq!(target, tterm_core::events::EventTarget::Any);
    payload
}

fn host_wire(payload: SshHostKeyPromptPayload) -> Value {
    wire(
        tterm_core::events::AppEvent::Ssh(tterm_core::events::ssh::SshEvent::HostKeyPrompt {
            tab_id: "p1-05-tab".into(),
            payload,
        }),
        "ssh-hostkey-prompt-p1-05-tab",
    )
}
fn auth_wire(payload: AuthPromptPayload) -> Value {
    wire(
        tterm_core::events::AppEvent::Ssh(tterm_core::events::ssh::SshEvent::AuthPrompt {
            tab_id: "p1-05-tab".into(),
            payload,
        }),
        "ssh-auth-prompt-p1-05-tab",
    )
}

#[test]
fn prompt_contract_expired_events_are_strings_with_legacy_names_and_any_target() {
    use tterm_core::events::{proxy::ProxyEvent, ssh::SshEvent, AppEvent};
    assert_eq!(
        wire(
            AppEvent::Ssh(SshEvent::HostKeyPromptExpired {
                tab_id: "p1-05-tab".into(),
                request_id: "host-request".into(),
            }),
            "ssh-hostkey-prompt-expired-p1-05-tab"
        ),
        json!("host-request")
    );
    assert_eq!(
        wire(
            AppEvent::Ssh(SshEvent::AuthPromptExpired {
                tab_id: "p1-05-tab".into(),
                request_id: "auth-request".into()
            }),
            "ssh-auth-prompt-expired-p1-05-tab"
        ),
        json!("auth-request")
    );
    assert_eq!(
        wire(
            AppEvent::Proxy(ProxyEvent::CommandPromptExpired("hash".into())),
            "proxy-command-prompt-expired"
        ),
        json!("hash")
    );
}
