//! P2-12: each terminal query has one responder over a real SSH channel.
//!
//! Each group is released by ordinary input and may contain several queries.
//! The server checks the complete transcript, including the final input fence.

use std::time::{Duration, Instant};

use gpui_kit::TestAppContext;
use gpui_kit::test::TestWindowExt as _;
use tterm_terminal::grid::WindowSize;

use super::server::{ShellScript, ShellServer};
use super::{direct_launch, harness, output, pump_until, trust_server};

const DA1_REPLY: &[u8] = b"\x1b[?64;1;2;4;6;9;15;21;22c";
const DA2_REPLY: &[u8] = b"\x1b[>0;276;0c";
const STATUS_REPLY: &[u8] = b"\x1b[0n";
const CPR_REPLY: &[u8] = b"\x1b[4;7R";
const FOREGROUND_COLOR_REPLY: &[u8] = b"\x1b]10;rgb:d7d7/dddd/e8e8\x07";
const BACKGROUND_COLOR_REPLY: &[u8] = b"\x1b]11;rgb:1b1b/1d1d/2323\x07";
const CURSOR_COLOR_REPLY: &[u8] = b"\x1b]12;rgb:7979/d8d8/ffff\x07";
const XTGETTCAP_CO_REPLY: &[u8] = b"\x1bP1+r436f3d323536\x1b\\";
const XTGETTCAP_MS_REPLY: &[u8] = b"\x1bP1+r4d73=5c455d35323b25703125733b25703225735c303037\x1b\\";
const START_GATE: &[u8] = b"P2-12-START";
const STATUS_GATE: &[u8] = b"P2-12-STATUS";
const WINDOW_GATE: &[u8] = b"P2-12-WINDOW";
const CPR_GATE: &[u8] = b"P2-12-CPR";
const MIXED_CPR_OSR_GATE: &[u8] = b"P2-12-MIXED-CPR-OSR";
const MIXED_OSR_CPR_GATE: &[u8] = b"P2-12-MIXED-OSR-CPR";
const CAPABILITIES_GATE: &[u8] = b"P2-12-CAPABILITIES";
const SPLIT_CAPABILITIES_GATE: &[u8] = b"P2-12-SPLIT-CAPABILITIES";
const SYNC_FORWARD_GATE: &[u8] = b"P2-12-SYNC-FORWARD";
const SYNC_REVERSE_GATE: &[u8] = b"P2-12-SYNC-REVERSE";
const COUNT_GATE: &[u8] = b"P2-12-COUNT";
const DONE_GATE: &[u8] = b"P2-12-DONE";
const END_MARKER: &str = "P2-12-END";
const COUNT_END_MARKER: &str = "P2-12-COUNT-END";
const DONE_ACK: &str = "P2-12-DONE-ACK";
const CPR_OSR_REPLY: &[u8] = b"\x1b[4;7R\x1b[0n";
const OSR_CPR_REPLY: &[u8] = b"\x1b[0n\x1b[4;7R";
const REPLY_DEADLINE: Duration = Duration::from_secs(20);
const STABLE: Duration = Duration::from_millis(100);

/// Waits for the exact transcript and fails as soon as it stops being a prefix.
fn expect_transcript(
    visual: &mut gpui_kit::VisualTestContext,
    server: &ShellServer,
    expected: &[u8],
    step: &str,
) {
    let deadline = Instant::now() + REPLY_DEADLINE;
    loop {
        visual.run_until_parked();
        let actual = server.inputs();
        assert!(
            expected.starts_with(&actual),
            "{step}: transcript diverged after {} bytes; expected {:?}, observed {:?}",
            expected.len(),
            String::from_utf8_lossy(expected),
            String::from_utf8_lossy(&actual)
        );
        if actual == expected {
            return;
        }
        if Instant::now() >= deadline {
            panic!(
                "{step}: expected {:?}, observed {:?}",
                String::from_utf8_lossy(expected),
                String::from_utf8_lossy(&actual)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Builds the reply Alacritty encodes for `CSI 14 t` from the live model size.
fn text_area_reply(size: WindowSize) -> Vec<u8> {
    let height = size.grid.screen_lines * usize::from(size.cell.height);
    let width = size.grid.columns * usize::from(size.cell.width);
    format!("\x1b[4;{height};{width}t").into_bytes()
}

fn query_script() -> ShellScript {
    ShellScript::new(vec![b"SAVED_SSH_READY\r\n".to_vec()])
        .after_input(START_GATE, vec![b"\x1b[".to_vec(), b"c".to_vec()])
        .after_input(STATUS_GATE, vec![b"\x1b[5n".to_vec()])
        .after_input(WINDOW_GATE, vec![b"\x1b[14t".to_vec()])
        .after_input(
            CPR_GATE,
            vec![
                b"\x1b[4;7H\x1b[6n".to_vec(),
                format!("\r\n{END_MARKER}\r\n").into_bytes(),
            ],
        )
        .after_input(
            MIXED_CPR_OSR_GATE,
            vec![b"\x1b[4;7H\x1b[6n\x1b[5n".to_vec()],
        )
        .after_input(
            MIXED_OSR_CPR_GATE,
            vec![b"\x1b[4;7H\x1b[5n\x1b[6n".to_vec()],
        )
        .after_input(
            CAPABILITIES_GATE,
            vec![b"\x1b[4;7H\x1b[6n\x1b[c\x1b[5n\x1b[>c\x1b[6n".to_vec()],
        )
        .after_input(
            SPLIT_CAPABILITIES_GATE,
            vec![
                b"\x1b[?6n\x1b[14t\x1bP+q43".to_vec(),
                b"6f\x1b".to_vec(),
                b"\\\x1b[6n".to_vec(),
            ],
        )
        .after_input(DONE_GATE, vec![format!("\r\n{DONE_ACK}\r\n").into_bytes()])
}

fn count_query_script() -> ShellScript {
    ShellScript::new(vec![b"SAVED_SSH_READY\r\n".to_vec()]).after_input(
        COUNT_GATE,
        vec![
            [
                b"\x1b[4;7H\x1b[6n\x1b[5n\x1b[c\x1b[>c".as_slice(),
                b"\x1b]10;?\x07\x1b]11;?\x07\x1b]12;?\x07",
                b"\x1bP+q436f\x1b\\\x1bP+q4d73\x1b\\",
            ]
            .concat(),
            format!("\r\n{COUNT_END_MARKER}\r\n").into_bytes(),
        ],
    )
}

fn reply_count(transcript: &[u8], reply: &[u8]) -> usize {
    transcript
        .windows(reply.len())
        .filter(|window| *window == reply)
        .count()
}

fn sync_query_script() -> ShellScript {
    ShellScript::new(vec![b"SAVED_SSH_READY\r\n".to_vec()])
        .after_input(
            SYNC_FORWARD_GATE,
            vec![
                b"\x1b[?2026h\x1b[4;7H\x1b[6n".to_vec(),
                b"\x1b[c".to_vec(),
                b"\x1b[9;3H\x1b[6n\x1b[?2026l".to_vec(),
                b"\x1b[2;5H\x1b[6n".to_vec(),
            ],
        )
        .after_input(
            SYNC_REVERSE_GATE,
            vec![
                b"\x1b[?2026h\x1b[c".to_vec(),
                b"\x1b[6;4H\x1b[6n".to_vec(),
                b"\x1b[>c".to_vec(),
                b"\x1b[?2026l\x1b[5n".to_vec(),
            ],
        )
        .after_input(DONE_GATE, vec![format!("\r\n{DONE_ACK}\r\n").into_bytes()])
}

#[gpui_kit::test]
fn core_and_model_queries_are_answered_once_and_in_order(cx: &mut TestAppContext) {
    let harness = harness(cx);
    let server = ShellServer::start_scripted(harness._runtime._runtime.handle(), query_script());
    let name = "Query Responder";
    trust_server("query-responder", name, &server);
    let root = harness.root.clone();
    let launch = direct_launch("query-responder", name, &server);
    let tab_id = harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace
                .open_saved_profile_launch(launch, window, cx)
                .expect("the saved launch should open one SSH tab")
        })
    });
    let (session, model) = harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        let session = workspace
            .tabs
            .get(tab_id)
            .and_then(|tab| tab.binding.as_ref())
            .and_then(|binding| binding.session())
            .expect("the saved launch runs a live session");
        (session.clone(), session.model())
    });
    pump_until(harness.visual, "the SSH shell to become ready", |_| {
        output(&model).contains("SAVED_SSH_READY")
    });
    assert_eq!(server.connections(), 1, "one loopback connection");
    assert_eq!(server.shells(), 1, "one interactive shell");

    let input = session.input();
    let mut expected = Vec::new();

    input(START_GATE);
    expected.extend_from_slice(START_GATE);
    expected.extend_from_slice(DA1_REPLY);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "split DA1 has exactly one core reply",
    );

    input(STATUS_GATE);
    expected.extend_from_slice(STATUS_GATE);
    expected.extend_from_slice(STATUS_REPLY);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "status has exactly one reply",
    );

    let window_reply = text_area_reply(model.snapshot().window_size());
    input(WINDOW_GATE);
    expected.extend_from_slice(WINDOW_GATE);
    expected.extend_from_slice(&window_reply);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "window size has exactly one model reply",
    );

    input(CPR_GATE);
    expected.extend_from_slice(CPR_GATE);
    expected.extend_from_slice(CPR_REPLY);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "a non-home CPR has exactly one model reply",
    );
    pump_until(harness.visual, "the parser to apply the end marker", |_| {
        output(&model).contains(END_MARKER)
    });

    // Packet order is the contract, not responder priority. The reverse case
    // fails if core replies are merely delayed behind every model reply.
    input(MIXED_CPR_OSR_GATE);
    expected.extend_from_slice(MIXED_CPR_OSR_GATE);
    expected.extend_from_slice(CPR_OSR_REPLY);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "CPR then OSR in one packet keeps request order",
    );

    input(MIXED_OSR_CPR_GATE);
    expected.extend_from_slice(MIXED_OSR_CPR_GATE);
    expected.extend_from_slice(OSR_CPR_REPLY);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "OSR then CPR in one packet keeps request order",
    );

    for (gate, replies) in [
        (
            CAPABILITIES_GATE,
            [
                CPR_REPLY,
                DA1_REPLY,
                STATUS_REPLY,
                b"\x1b[>0;276;0c",
                CPR_REPLY,
            ]
            .concat(),
        ),
        (
            SPLIT_CAPABILITIES_GATE,
            [
                b"\x1b[?1;1R".as_slice(),
                &window_reply,
                b"\x1bP1+r436f3d323536\x1b\\",
                CPR_REPLY,
            ]
            .concat(),
        ),
    ] {
        input(gate);
        expected.extend_from_slice(gate);
        expected.extend_from_slice(&replies);
        expect_transcript(
            harness.visual,
            &server,
            &expected,
            "mixed SSH capabilities and model queries",
        );
    }

    input(DONE_GATE);
    expected.extend_from_slice(DONE_GATE);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "the final ordinary-input fence closes the transcript",
    );
    pump_until(harness.visual, "the final fence acknowledgement", |_| {
        output(&model).contains(DONE_ACK)
    });
    std::thread::sleep(STABLE);
    harness.visual.run_until_parked();
    assert_eq!(
        server.inputs(),
        expected,
        "a reply may not be duplicated after the acknowledged fence"
    );
    assert_eq!(
        server.script_remaining(),
        0,
        "every scripted query should have been requested"
    );
    harness.visual.update(|window, _| window.remove_window());
}

/// G2 evidence: count each reply kind in the full loopback server transcript.
#[gpui_kit::test]
fn each_query_kind_is_answered_exactly_once(cx: &mut TestAppContext) {
    let harness = harness(cx);
    let server =
        ShellServer::start_scripted(harness._runtime._runtime.handle(), count_query_script());
    let name = "Query Counts";
    trust_server("query-counts", name, &server);
    let root = harness.root.clone();
    let launch = direct_launch("query-counts", name, &server);
    let tab_id = harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace
                .open_saved_profile_launch(launch, window, cx)
                .expect("the saved launch should open one SSH tab")
        })
    });
    let (session, model) = harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        let session = workspace
            .tabs
            .get(tab_id)
            .and_then(|tab| tab.binding.as_ref())
            .and_then(|binding| binding.session())
            .expect("the saved launch runs a live session");
        (session.clone(), session.model())
    });
    pump_until(harness.visual, "the SSH shell to become ready", |_| {
        output(&model).contains("SAVED_SSH_READY")
    });

    session.input()(COUNT_GATE);
    pump_until(harness.visual, "the count fence after every query", |_| {
        output(&model).contains(COUNT_END_MARKER)
    });
    std::thread::sleep(STABLE);
    harness.visual.run_until_parked();
    assert_eq!(
        server.script_remaining(),
        0,
        "the server should have released every scripted query"
    );

    let replies: [(&str, &str, &[u8]); 9] = [
        ("CPR", "model", CPR_REPLY),
        ("DSR", "model", STATUS_REPLY),
        ("DA1", "SSH", DA1_REPLY),
        ("DA2", "SSH", DA2_REPLY),
        ("OSC 10 foreground", "model", FOREGROUND_COLOR_REPLY),
        ("OSC 11 background", "model", BACKGROUND_COLOR_REPLY),
        ("OSC 12 cursor", "model", CURSOR_COLOR_REPLY),
        ("XTGETTCAP Co", "SSH", XTGETTCAP_CO_REPLY),
        ("XTGETTCAP Ms", "SSH", XTGETTCAP_MS_REPLY),
    ];
    let inputs = server.inputs();
    let mut expected = COUNT_GATE.to_vec();
    for (_, _, reply) in &replies {
        expected.extend_from_slice(reply);
    }
    assert_eq!(
        inputs, expected,
        "the full server transcript carries the gate and each reply in wire order"
    );
    for (label, responder, reply) in replies {
        let count = reply_count(&inputs, reply);
        eprintln!("P2-12 query={label} responder={responder} count={count}");
        assert_eq!(
            count, 1,
            "{label} is answered by {responder} and must appear exactly once"
        );
    }
    harness.visual.update(|window, _| window.remove_window());
}

/// P2-12 fourth batch: fixed and model replies keep wire order across BSU/ESU.
#[gpui_kit::test]
fn synchronized_output_keeps_query_replies_in_wire_order(cx: &mut TestAppContext) {
    let harness = harness(cx);
    let server =
        ShellServer::start_scripted(harness._runtime._runtime.handle(), sync_query_script());
    let name = "Synchronized Queries";
    trust_server("synchronized-queries", name, &server);
    let root = harness.root.clone();
    let launch = direct_launch("synchronized-queries", name, &server);
    let tab_id = harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace
                .open_saved_profile_launch(launch, window, cx)
                .expect("the saved launch should open one SSH tab")
        })
    });
    let (session, model) = harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        let session = workspace
            .tabs
            .get(tab_id)
            .and_then(|tab| tab.binding.as_ref())
            .and_then(|binding| binding.session())
            .expect("the saved launch runs a live session");
        (session.clone(), session.model())
    });
    pump_until(harness.visual, "the SSH shell to become ready", |_| {
        output(&model).contains("SAVED_SSH_READY")
    });

    let input = session.input();
    let mut expected = Vec::new();

    // The block straddles four channel-data packets. The fixed DA1 lies
    // between the two CPRs and must come back between them too.
    input(SYNC_FORWARD_GATE);
    expected.extend_from_slice(SYNC_FORWARD_GATE);
    expected.extend_from_slice(
        [
            b"\x1b[4;7R".as_slice(),
            DA1_REPLY,
            b"\x1b[9;3R".as_slice(),
            b"\x1b[2;5R".as_slice(),
        ]
        .concat()
        .as_slice(),
    );
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "model replies before and after a fixed reply keep stream order",
    );

    // A fixed reply first, then a model reply, then a fixed reply.
    input(SYNC_REVERSE_GATE);
    expected.extend_from_slice(SYNC_REVERSE_GATE);
    expected.extend_from_slice(
        [
            DA1_REPLY,
            b"\x1b[6;4R".as_slice(),
            b"\x1b[>0;276;0c".as_slice(),
            b"\x1b[0n".as_slice(),
        ]
        .concat()
        .as_slice(),
    );
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "fixed replies before later model replies keep stream order",
    );

    input(DONE_GATE);
    expected.extend_from_slice(DONE_GATE);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "the final input fence closes the synchronized transcript",
    );
    pump_until(harness.visual, "the final fence acknowledgement", |_| {
        output(&model).contains(DONE_ACK)
    });
    std::thread::sleep(STABLE);
    harness.visual.run_until_parked();
    assert_eq!(
        server.inputs(),
        expected,
        "a synchronized reply may not be reordered or duplicated after the fence"
    );
    assert_eq!(
        server.script_remaining(),
        0,
        "every scripted synchronized query should have been requested"
    );
    harness.visual.update(|window, _| window.remove_window());
}
