//! Tests for the OSC 52 slot, drop scoping and paste delivery helpers.

use std::sync::Arc;

use tterm_core::core::session::types::PtyConnectionOptions;

use super::local_drop_text;
use super::store::{ClipboardWrites, MAX_CLIPBOARD_STORE_BYTES};

#[test]
fn keeps_only_the_latest_write_and_preserves_unicode() {
    let writes = Arc::new(ClipboardWrites::new(true));
    assert!(writes.store("first".to_string()));
    assert!(writes.store("你好，世界\n".to_string()));
    assert_eq!(writes.take().as_deref(), Some("你好，世界\n"));
    assert_eq!(writes.take(), None);
}

#[test]
fn rejects_a_disabled_store_and_an_empty_payload() {
    let disabled = ClipboardWrites::new(false);
    assert!(!disabled.store("text".to_string()));
    assert_eq!(disabled.take(), None);

    let enabled = ClipboardWrites::new(true);
    assert!(!enabled.store(String::new()));
    assert_eq!(enabled.take(), None);
}

#[test]
fn rejects_a_payload_over_the_byte_bound_instead_of_holding_it() {
    let writes = ClipboardWrites::new(true);
    assert!(!writes.store("x".repeat(MAX_CLIPBOARD_STORE_BYTES + 1)));
    assert_eq!(writes.take(), None);
    assert!(writes.store("x".repeat(MAX_CLIPBOARD_STORE_BYTES)));
    assert!(writes.take().is_some());
}

#[test]
fn wakeups_coalesce_while_the_ui_is_behind() {
    let writes = ClipboardWrites::new(true);
    let watch = writes.subscribe();
    assert!(writes.store("one".to_string()));
    assert!(writes.store("two".to_string()));
    assert!(writes.store("three".to_string()));
    assert!(watch.has_changed().expect("watch open"));
    assert_eq!(writes.take().as_deref(), Some("three"));
}

#[test]
fn only_local_tabs_get_a_drop_formatter() {
    let local = PtyConnectionOptions {
        connection_type: Some("terminal".to_string()),
        ..PtyConnectionOptions::default()
    };
    assert!(local_drop_text(&local).is_some());

    let ssh = PtyConnectionOptions {
        connection_type: Some("ssh".to_string()),
        ..PtyConnectionOptions::default()
    };
    assert!(local_drop_text(&ssh).is_none());
}

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, ClipboardItem, Context, Entity, IntoElement, ParentElement,
    Render, Styled, TestAppContext, Window, div,
};
use tterm_core::i18n::language::Language;
use tterm_terminal::grid::{CellSize as TerminalCellSize, GridSize as TerminalGridSize};
use tterm_terminal::terminal::{Config as TerminalConfig, Terminal as TerminalModel};
use tterm_terminal_view::view::paste::{PasteGate, PasteRequest};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};
use tterm_ui::components::dialog::DialogAction;

use super::{PasteController, PasteTarget};
use crate::locale::Locale;

struct Probe {
    controller: Entity<PasteController>,
    locale: Entity<Locale>,
    delivered: Rc<Cell<bool>>,
    live: Rc<Cell<bool>>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.controller.clone())
    }
}

fn harness(cx: &mut TestAppContext) -> (Entity<Probe>, &mut gpui_kit::VisualTestContext) {
    harness_with_confirmation(cx, true)
}

fn harness_with_confirmation(
    cx: &mut TestAppContext,
    confirm_multiline: bool,
) -> (Entity<Probe>, &mut gpui_kit::VisualTestContext) {
    cx.update(gpui_kit::init);
    let mut built = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let locale = cx.new(|_| Locale::new(Language::En));
        let controller =
            cx.new(|cx| PasteController::for_test(cx, locale.clone(), confirm_multiline));
        let probe = cx.new(|_| Probe {
            controller,
            locale,
            delivered: Rc::new(Cell::new(false)),
            live: Rc::new(Cell::new(true)),
        });
        built = Some(probe.clone());
        gpui_kit::base::Root::new(probe, window, cx)
    });
    (built.expect("probe"), visual)
}

fn target(probe: &Probe, expected: &'static str) -> PasteTarget {
    let delivered = Rc::clone(&probe.delivered);
    let live = Rc::clone(&probe.live);
    PasteTarget {
        session_nonce: 7,
        is_current: Rc::new(move |nonce| nonce == 7 && live.get()),
        delivery: Rc::new(move |_, text| {
            assert_eq!(text, expected);
            delivered.set(true);
        }),
    }
}

#[gpui_kit::test]
fn a_multiline_paste_waits_for_confirmation_then_delivers(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx);
    let controller = visual.update(|_, cx| probe.read(cx).controller.clone());
    let dialog = visual.update(|window, cx| {
        let target = target(probe.read(cx), "one\ntwo");
        assert!(controller.update(cx, |controller, cx| {
            controller.request("one\ntwo".to_string(), false, target, window, cx)
        }));
        let dialog = controller
            .read(cx)
            .dialog
            .clone()
            .expect("the confirmation dialog opened");
        assert!(
            !probe.read(cx).delivered.get(),
            "no delivery before the button"
        );
        assert!(
            dialog
                .read(cx)
                .options
                .description
                .as_deref()
                .is_some_and(|text| text.contains("one\ntwo"))
        );
        dialog
    });
    visual.update(|window, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.close(Some(DialogAction::Confirm), window, cx);
        });
    });
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert!(probe.read(cx).delivered.get());
        assert!(controller.read(cx).dialog.is_none());
    });
}

#[gpui_kit::test]
fn cancel_and_a_stale_session_send_nothing(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx);
    let controller = visual.update(|_, cx| probe.read(cx).controller.clone());
    let dialog = visual.update(|window, cx| {
        let target = target(probe.read(cx), "never");
        controller.update(cx, |controller, cx| {
            controller.request("one\ntwo".to_string(), false, target, window, cx);
        });
        controller.read(cx).dialog.clone().expect("dialog")
    });
    visual.update(|window, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.close(Some(DialogAction::Cancel), window, cx);
        });
    });
    visual.run_until_parked();
    visual.update(|_, cx| assert!(!probe.read(cx).delivered.get()));

    visual.update(|_, cx| probe.update(cx, |probe, _| probe.live.set(false)));
    let dialog = visual.update(|window, cx| {
        let target = target(probe.read(cx), "never");
        assert!(controller.update(cx, |controller, cx| {
            controller.request("one\ntwo".to_string(), false, target, window, cx)
        }));
        controller.read(cx).dialog.clone().expect("dialog")
    });
    visual.update(|window, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.close(Some(DialogAction::Confirm), window, cx);
        });
    });
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert!(
            !probe.read(cx).delivered.get(),
            "a closed or replaced session rejects the pending paste"
        )
    });
}

#[gpui_kit::test]
fn a_single_line_or_bracketed_request_falls_back_to_the_view(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx);
    let controller = visual.update(|_, cx| probe.read(cx).controller.clone());
    visual.update(|window, cx| {
        let single = target(probe.read(cx), "one line");
        assert!(!controller.update(cx, |controller, cx| {
            controller.request("one line".to_string(), false, single, window, cx)
        }));
        let bracketed = target(probe.read(cx), "one\ntwo");
        assert!(!controller.update(cx, |controller, cx| {
            controller.request("one\ntwo".to_string(), true, bracketed, window, cx)
        }));
        assert!(controller.read(cx).dialog.is_none());
    });
}

#[gpui_kit::test]
fn confirmation_disabled_falls_back_for_a_multiline_request(cx: &mut TestAppContext) {
    let (probe, visual) = harness_with_confirmation(cx, false);
    let controller = visual.update(|_, cx| probe.read(cx).controller.clone());
    visual.update(|window, cx| {
        let target = target(probe.read(cx), "one\ntwo");
        assert!(!controller.update(cx, |controller, cx| {
            controller.request("one\ntwo".to_string(), false, target, window, cx)
        }));
        assert!(controller.read(cx).dialog.is_none());
    });
}

#[gpui_kit::test]
fn an_open_dialog_locks_out_a_later_single_line_paste(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx);
    let controller = visual.update(|_, cx| probe.read(cx).controller.clone());
    let dialog = visual.update(|window, cx| {
        let first = target(probe.read(cx), "first\nsecond");
        assert!(controller.update(cx, |controller, cx| {
            controller.request("first\nsecond".to_string(), false, first, window, cx)
        }));
        let dialog = controller.read(cx).dialog.clone().expect("dialog");
        let second = target(probe.read(cx), "never");
        assert!(
            controller.update(cx, |controller, cx| {
                controller.request("single line".to_string(), false, second, window, cx)
            }),
            "the open modal must consume the later request"
        );
        assert_eq!(
            controller
                .read(cx)
                .pending
                .as_ref()
                .map(|pending| pending.text.as_str()),
            Some("first\nsecond")
        );
        dialog
    });
    visual.update(|window, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.close(Some(DialogAction::Confirm), window, cx);
        });
    });
    visual.run_until_parked();
    visual.update(|_, cx| assert!(probe.read(cx).delivered.get()));
}

#[gpui_kit::test]
fn changing_the_locale_refreshes_an_open_dialog(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx);
    let controller = visual.update(|_, cx| probe.read(cx).controller.clone());
    let dialog = visual.update(|window, cx| {
        let target = target(probe.read(cx), "one\ntwo");
        controller.update(cx, |controller, cx| {
            controller.request("one\ntwo".to_string(), false, target, window, cx);
        });
        let dialog = controller.read(cx).dialog.clone().expect("dialog");
        assert_eq!(dialog.read(cx).options.title, "Paste multiple lines?");
        dialog
    });
    visual.update(|_, cx| {
        probe.update(cx, |probe, cx| {
            probe
                .locale
                .update(cx, |locale, cx| locale.set_language(Language::Zh, cx));
        });
    });
    visual.run_until_parked();
    visual.update(|_, cx| {
        let dialog = dialog.read(cx);
        assert_eq!(dialog.options.title, "粘贴多行内容？");
        assert_eq!(dialog.options.confirm_text, "粘贴");
    });
}

#[gpui_kit::test]
fn dropping_the_controller_cancels_the_clipboard_watcher(cx: &mut TestAppContext) {
    let writes = Arc::new(ClipboardWrites::new(true));
    let controller = cx.update(|cx| {
        let locale = cx.new(|_| Locale::new(Language::En));
        cx.new(|cx| PasteController::for_test_with_clipboard(cx, locale, true, Arc::clone(&writes)))
    });
    assert!(writes.store("before drop".to_string()));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("before drop".to_string())
    );

    drop(controller);
    assert!(writes.store("after drop".to_string()));
    cx.run_until_parked();
    assert_eq!(
        writes.take().as_deref(),
        Some("after drop"),
        "the dropped controller leaves no detached consumer"
    );
}

struct ViewProbe {
    controller: Entity<PasteController>,
    view: Entity<TerminalView>,
}

impl Render for ViewProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(self.view.clone())
            .child(self.controller.clone())
    }
}

struct ViewHarness {
    window: AnyWindowHandle,
    probe: Entity<ViewProbe>,
    bytes: Rc<std::cell::RefCell<Vec<u8>>>,
    model: Arc<TerminalModel>,
}

fn view_harness(cx: &mut TestAppContext) -> ViewHarness {
    cx.update(gpui_kit::init);
    let bytes = Rc::new(std::cell::RefCell::new(Vec::new()));
    let captured = Rc::clone(&bytes);
    let model = Arc::new(TerminalModel::new(
        TerminalConfig::default(),
        TerminalGridSize::new(80, 24),
        TerminalCellSize::new(8, 16),
    ));
    let view_model = Arc::clone(&model);
    let (window, probe) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |_, cx| {
            let locale = cx.new(|_| Locale::new(Language::En));
            let controller = cx.new(|cx| PasteController::for_test(cx, locale, true));
            let gate_controller = controller.downgrade();
            let gate: PasteGate = Rc::new(move |request, window, cx| {
                let Some(controller) = gate_controller.upgrade() else {
                    return true;
                };
                let PasteRequest {
                    text,
                    bracketed,
                    delivery,
                } = request;
                controller.update(cx, |controller, cx| {
                    controller.request(
                        text,
                        bracketed,
                        PasteTarget {
                            session_nonce: 1,
                            is_current: Rc::new(|_| true),
                            delivery,
                        },
                        window,
                        cx,
                    )
                })
            });
            let output = Rc::clone(&captured);
            let view = cx.new(|cx| {
                TerminalView::new(
                    cx,
                    view_model,
                    TerminalViewConfig {
                        input: Some(Rc::new(move |input| {
                            output.borrow_mut().extend_from_slice(input);
                        })),
                        paste_gate: Some(gate),
                        ..TerminalViewConfig::default()
                    },
                )
            });
            cx.new(|_| ViewProbe { controller, view })
        })
        .expect("open clipboard view")
    });
    cx.update_window(window, |_, window, cx| {
        let view = probe.read(cx).view.clone();
        let focus = view.read(cx).focus_handle();
        window.focus(&focus, cx);
        window.render_frame(cx);
    })
    .expect("draw clipboard view");
    ViewHarness {
        window,
        probe,
        bytes,
        model,
    }
}

#[gpui_kit::test]
fn the_production_view_gate_delivers_a_plain_paste_after_its_borrow(cx: &mut TestAppContext) {
    let harness = view_harness(cx);
    cx.update(|cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("plain paste".to_string()));
    });
    cx.simulate_keystrokes(
        harness.window,
        if cfg!(target_os = "macos") {
            "cmd-v"
        } else {
            "ctrl-shift-v"
        },
    );
    assert_eq!(&*harness.bytes.borrow(), b"plain paste");
    assert!(
        harness
            .probe
            .read_with(cx, |probe, cx| probe.controller.read(cx).dialog.is_none())
    );
}

#[gpui_kit::test]
fn confirmed_paste_reads_the_live_bracketed_mode(cx: &mut TestAppContext) {
    let harness = view_harness(cx);
    cx.update(|cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("one\ntwo".to_string()));
    });
    cx.simulate_keystrokes(
        harness.window,
        if cfg!(target_os = "macos") {
            "cmd-v"
        } else {
            "ctrl-shift-v"
        },
    );
    assert!(harness.bytes.borrow().is_empty());
    let controller = harness
        .probe
        .read_with(cx, |probe, _| probe.controller.clone());
    let dialog = controller.read_with(cx, |controller, _| {
        controller.dialog.clone().expect("confirmation dialog")
    });
    harness.model.write(b"\x1b[?2004h");
    cx.update_window(harness.window, |_, window, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.close(Some(DialogAction::Confirm), window, cx);
        });
    })
    .expect("confirm the paste");
    cx.run_until_parked();
    assert_eq!(&*harness.bytes.borrow(), b"\x1b[200~one\rtwo\x1b[201~");
}

#[cfg(windows)]
#[test]
fn windows_drop_text_quotes_for_the_local_powershell_without_pressing_enter() {
    let local = PtyConnectionOptions {
        connection_type: Some("terminal".to_string()),
        terminal_shell: Some("pwsh".to_string()),
        ..PtyConnectionOptions::default()
    };
    let formatter = local_drop_text(&local).expect("local drop formatter");
    let text = formatter(&[
        r"C:\Program Files\a b.txt".to_string(),
        r"C:\tmp\it's".to_string(),
        r"C:\文档\x".to_string(),
    ]);
    assert!(
        text.ends_with(' '),
        "the next argument can follow: {text:?}"
    );
    assert!(text.contains(r"'C:\Program Files\a b.txt'"));
    assert!(text.contains(r"'C:\tmp\it''s'"));
    assert!(text.contains(r"C:\文档\x"));
    assert!(!text.contains('\r') && !text.contains('\n'));
}

#[cfg(not(windows))]
#[test]
fn posix_drop_text_escapes_spaces_quotes_and_unicode_without_pressing_enter() {
    let local = PtyConnectionOptions {
        connection_type: Some("terminal".to_string()),
        ..PtyConnectionOptions::default()
    };
    let formatter = local_drop_text(&local).expect("local drop formatter");
    let text = formatter(&[
        "/tmp/My File.txt".to_string(),
        "/tmp/it's".to_string(),
        "/tmp/文档".to_string(),
    ]);
    assert!(
        text.ends_with(' '),
        "the next argument can follow: {text:?}"
    );
    assert!(text.contains(r"/tmp/My\ File.txt"));
    assert!(text.contains(r"/tmp/it\'s"));
    assert!(text.contains("/tmp/文档"));
    assert!(!text.contains('\r') && !text.contains('\n'));
}
