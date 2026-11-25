//! Query state must survive the integrated shell's exec acknowledgement.

use gpui_kit::TestAppContext;

use super::server::{ShellScript, ShellServer};
use super::{direct_launch, harness, output, pump_until, trust_server};

#[gpui_kit::test]
fn early_and_live_shell_output_share_query_state(cx: &mut TestAppContext) {
    let harness = harness(cx);
    let gate = b"P2-12-FENCE";
    let script = ShellScript::new(vec![b"c\r\nLIVE\r\n".to_vec()])
        .before_exec_success(b"EARLY\x1b[3;9H\x1b[6n\x1b[".to_vec())
        .after_input(gate, vec![b"FENCED".to_vec()]);
    let server = ShellServer::start_scripted(harness._runtime._runtime.handle(), script);
    let id = "early-query";
    let name = "Early Query";
    trust_server(id, name, &server);
    let mut launch = direct_launch(id, name, &server);
    launch.connection.ssh_shell_integration = Some(true);
    let root = harness.root;
    let session = harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            let tab_id = workspace
                .open_saved_profile_launch(launch, window, cx)
                .unwrap();
            workspace
                .tabs
                .get(tab_id)
                .unwrap()
                .binding
                .as_ref()
                .unwrap()
                .session()
                .unwrap()
        })
    });
    let model = session.model();
    pump_until(
        harness.visual,
        "ordinary channel data after exec success",
        |_| output(&model).contains("LIVE"),
    );
    pump_until(
        harness.visual,
        "the device attributes reply on the wire",
        |_| server.inputs().contains(&b'c'),
    );
    session.input()(gate);
    pump_until(
        harness.visual,
        "the input fence after both query replies",
        |_| output(&model).contains("FENCED"),
    );
    let screen = output(&model);
    assert_eq!(screen.matches("EARLY").count(), 1);
    assert_eq!(screen.matches("LIVE").count(), 1);
    assert_eq!(server.shells(), 1);
    assert_eq!(
        server.inputs(),
        [b"\x1b[3;9R\x1b[?64;1;2;4;6;9;15;21;22c".as_slice(), gate].concat(),
        "both the buffered CPR and the DA split across exec success are answered exactly once"
    );
    harness.visual.update(|window, _| window.remove_window());
}
