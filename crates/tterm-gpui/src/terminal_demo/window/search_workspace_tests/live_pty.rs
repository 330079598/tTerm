//! Searches retained output from a real non-login Unix shell through the workspace.

use std::time::{Duration, Instant};

use super::*;

const PROGRAM: &str = r#"
set -eu
cd /
test -t 0 && test -t 1 || exit 1
stty -echo
printf 'ready-for-input\n'
IFS= read -r request
test "$request" = run
for section in first second; do
    printf 'needle-%s\n' "$section"
    line=0
    while [ "$line" -lt 128 ]; do
        printf 'filler-%s\n' "$line"
        line=$((line+1))
    done
done
printf 'finished-output\n'
IFS= read -r reply
printf 'shell-received:%s\n' "$reply"
"#;

fn visible(model: &Terminal, text: &str) -> bool {
    model.snapshot().rows().iter().any(|row| {
        let line: String = row.cells().iter().map(|cell| cell.ch).collect();
        line.trim_end() == text
    })
}

fn wait_for_line(model: &Terminal, text: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !visible(model, text) {
        assert!(Instant::now() < deadline, "missing shell output: {text}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[gpui_kit::test]
fn find_reveals_retained_live_pty_output_and_returns_input_to_the_shell(cx: &mut TestAppContext) {
    let command = CommandSpec::Program {
        program: "/usr/bin/env".into(),
        args: [
            "-i",
            "PATH=/usr/bin:/bin",
            "HOME=/dev/null",
            "/bin/sh",
            "-c",
            PROGRAM,
        ]
        .map(str::to_owned)
        .into(),
    };
    with_command(cx, AppConfig::default(), command, |root, visual| {
        let model = visual.update(|_, cx| {
            root.read(cx)
                .tabs
                .active()
                .binding
                .as_ref()
                .unwrap()
                .session()
                .unwrap()
                .model()
        });
        wait_for_line(&model, "ready-for-input");
        press(visual, "r u n enter");
        wait_for_line(&model, "finished-output");
        assert_eq!(model.snapshot().display_offset(), 0);
        assert!(!visible(&model, "needle-first"));
        assert!(!visible(&model, "needle-second"));

        press(visual, &default_find());
        press(visual, "n e e d l e");
        visual
            .cx
            .executor()
            .advance_clock(Duration::from_millis(250));
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.render_frame(cx);
            let bar = active_search(&root, cx).expect("find opened the search bar");
            assert_eq!(bar.read(cx).result().unwrap().match_count(), 2);
            assert_eq!(bar.read(cx).active_index(), Some(0));
        });
        assert!(model.snapshot().display_offset() > 0);
        assert!(visible(&model, "needle-first"));
        press(visual, "enter");
        assert!(visible(&model, "needle-second"));
        assert!(!visible(&model, "needle-first"));

        press(visual, "escape");
        visual.update(|window, cx| {
            assert!(!search_is_open(&root, cx));
            assert!(
                root.read(cx)
                    .tabs
                    .active()
                    .view
                    .read(cx)
                    .focus_handle()
                    .is_focused(window)
            );
        });
        press(visual, "b a c k enter");
        wait_for_line(&model, "shell-received:back");
    });
}
