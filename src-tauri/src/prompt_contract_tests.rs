//! P1-05 baselines captured before prompt payloads or transports are migrated.

use crate::proxy::approval::ProxyCommandPrompt;
use crate::ssh::auth::{AuthPromptField, AuthPromptPayload};
use crate::ssh::store::SshHostKeyPromptPayload;
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
    assert_eq!(
        serde_json::to_value(host_key("p1-05", None)).unwrap(),
        expected
    );
    let mut changed = expected;
    changed["reason"] = json!("mismatch");
    changed["knownFingerprint"] = json!("SHA256:old");
    assert_eq!(
        serde_json::to_value(host_key("p1-05", Some("SHA256:old"))).unwrap(),
        changed
    );
}

#[test]
fn prompt_contract_jump_host_uses_synthetic_profile_without_hop_fields() {
    let profile = format!(
        "{}test.invalid:22505",
        crate::ssh::store::JUMP_HOST_PROFILE_PREFIX
    );
    assert_eq!(
        serde_json::to_value(host_key(&profile, None)).unwrap(),
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
            serde_json::to_value(auth(kind)).unwrap(),
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
        serde_json::to_value(payload).unwrap(),
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
        serde_json::to_value(sudo).unwrap(),
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
        serde_json::to_value(&prompt).unwrap(),
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

// These assertions tie the fixture names and scopes to the still-unmigrated
// call sites. C3 replaces them with executable typed-event wire assertions.
fn compact(source: &str) -> String {
    source.split_whitespace().collect()
}

#[test]
fn prompt_contract_legacy_ssh_send_sites_use_any_target_and_tab_names() {
    for source in [
        include_str!("ssh/types.rs"),
        include_str!("ssh/jump/handler.rs"),
    ] {
        let source = compact(source);
        assert!(source.contains("format!(\"ssh-hostkey-prompt-{}\",self.tab_id)"));
        assert!(source.contains(".emit_to(tauri::EventTarget::any(),&event_name,payload)"));
    }
    let source = compact(include_str!("ssh/auth/prompter.rs"));
    assert!(source.contains("format!(\"ssh-auth-prompt-{tab_id}\")"));
    assert!(source.contains(".emit_to(tauri::EventTarget::any(),&event_name,payload)"));
    assert!(source.contains(concat!(
        ".emit_to(tauri::EventTarget::any(),",
        "&format!(\"ssh-auth-prompt-expired-{tab_id}\"),request_id,)"
    )));
}

#[test]
fn prompt_contract_legacy_proxy_send_sites_use_any_target_and_static_names() {
    let source = compact(include_str!("proxy/approval.rs"));
    assert!(source.contains("constPROMPT_EVENT:&str=\"proxy-command-prompt\";"));
    assert!(source.contains("constPROMPT_EXPIRED_EVENT:&str=\"proxy-command-prompt-expired\";"));
    assert!(source.contains(".emit_to(tauri::EventTarget::any(),PROMPT_EVENT,prompt)"));
    assert!(source.contains(".emit_to(tauri::EventTarget::any(),PROMPT_EXPIRED_EVENT,&request_id)"));
}
