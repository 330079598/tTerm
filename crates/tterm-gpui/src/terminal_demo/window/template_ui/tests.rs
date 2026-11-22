use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{TestAppContext, VisualTestContext};
use tokio::sync::mpsc::UnboundedReceiver;
use tterm_core::config::AppConfig;
use tterm_core::events::AppEvent;
use tterm_core::i18n::keys;
use tterm_core::workspace::pane_templates::LayoutShape;
use tterm_core::workspace::tab_groups::{PaneNode, SplitOrientation};

use super::super::split_state::Edge;
use super::super::*;
use super::*;
use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};

type TemplateWrites = Arc<Mutex<Vec<Vec<ConfigTemplate>>>>;

fn exec_marker() -> CommandSpec {
    #[cfg(windows)]
    let (program, args) = (
        "cmd.exe",
        vec!["/d".into(), "/q".into(), "/k".into(), "echo RUNONCE".into()],
    );
    #[cfg(not(windows))]
    let (program, args) = (
        "/bin/sh",
        vec!["-c".into(), "echo RUNONCE; exec cat".into()],
    );
    CommandSpec::Program {
        program: program.into(),
        args,
    }
}

fn make_workspace(
    runtime: &GpuiRuntime,
    events: UnboundedReceiver<AppEvent>,
    command: CommandSpec,
    config: AppConfig,
    writes: TemplateWrites,
    cx: &mut Context<Prototype>,
) -> Prototype {
    let keymap = Rc::new(KeymapRouter::new(&config.keymap, cfg!(target_os = "macos")));
    let locale = cx.new(|_| Locale::new(Language::En));
    LocaleGlobal::set(cx, locale.clone());
    let handle = runtime._runtime.handle();
    let queue = state::SaveQueue::start(handle, Arc::new(|_| Ok(())));
    let geometry = cx.new(|_| WindowGeometry::new(None, Default::default(), queue));
    let config = cx.new(|_| {
        ConfigState::with_writers(
            config,
            handle,
            Arc::new(|_| Ok(())),
            Arc::new(move |templates| {
                if let Ok(mut writes) = writes.lock() {
                    writes.push(templates);
                }
                Ok(())
            }),
        )
    });
    Prototype::new(
        cx,
        PrototypeStartup {
            command,
            workspace: session_workspace::test_startup(handle),
            sessions: Rc::new(SessionManager::new(
                runtime.services.clone(),
                Default::default(),
            )),
            events,
            keyboard: encoder_config(cfg!(target_os = "macos"), false),
            keymap,
            locale,
            config,
            theme: background::theme(None),
            background: background::content(None),
        },
        geometry,
    )
}

fn workspace(
    cx: &mut TestAppContext,
    command: CommandSpec,
    config: AppConfig,
    writes: TemplateWrites,
) -> (GpuiRuntime, Entity<Prototype>, &mut VisualTestContext) {
    let mut runtime = test_runtime();
    let events = std::mem::replace(
        &mut runtime.events,
        tokio::sync::mpsc::unbounded_channel().1,
    );
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        tterm_terminal_view::view::bind_keys(cx);
    });
    let (root, visual) =
        cx.add_window_view(|_, cx| make_workspace(&runtime, events, command, config, writes, cx));
    (runtime, root, visual)
}

fn open_picker(visual: &mut VisualTestContext, root: &Entity<Prototype>) {
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.open_layout_templates(window, cx);
        });
        window.render_frame(cx);
    });
}

fn click(visual: &mut VisualTestContext, selector: &'static str) {
    visual.update(|window, cx| window.render_frame(cx));
    let bounds = visual
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} should be laid out"));
    let center = bounds.center();
    visual.simulate_mouse_down(center, MouseButton::Left, Default::default());
    visual.simulate_mouse_up(center, MouseButton::Left, Default::default());
    visual.update(|window, cx| window.render_frame(cx));
}

fn two_pane_shape() -> LayoutShape {
    LayoutShape::Split {
        orientation: SplitOrientation::Horizontal,
        children: vec![LayoutShape::Pane, LayoutShape::Pane],
        sizes: vec![0.5, 0.5],
    }
}

fn config_template(id: &str, name: &str) -> ConfigTemplate {
    ConfigTemplate {
        id: id.to_string(),
        name: name.to_string(),
        root: serde_json::to_value(two_pane_shape()).expect("the test shape serializes"),
    }
}

fn with_templates(templates: Vec<ConfigTemplate>) -> AppConfig {
    let mut config = AppConfig::default();
    config.panes.templates = templates;
    config
}

#[gpui_kit::test]
fn builtin_card_applies_to_the_original_tab_and_closes(cx: &mut TestAppContext) {
    let writes = TemplateWrites::default();
    let (_runtime, root, visual) = workspace(cx, CommandSpec::Shell, AppConfig::default(), writes);
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, CommandSpec::Shell, &workspace.ui.locale, cx)
            });
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Right, window, cx);
            workspace.select_tab(1, window, cx);
            workspace.open_layout_templates(window, cx);
            workspace.new_tab(window, cx);
        });
        window.render_frame(cx);
    });
    click(visual, "layout-template-builtin-builtin-rows-2");
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            assert!(!workspace.ui.template_ui.read(cx).open);
            assert_eq!(workspace.tabs.active_id(), 1);
            assert!(!workspace.splits.is_split(3));
            match workspace.splits.root(1) {
                Some(PaneNode::Split {
                    orientation,
                    children,
                    ..
                }) => {
                    assert_eq!(*orientation, SplitOrientation::Vertical);
                    assert_eq!(children.len(), 2);
                }
                _ => panic!("the template should stack the original tab's panes"),
            }
        });
        window.remove_window();
    });
}

#[gpui_kit::test]
fn disabled_card_keeps_the_picker_open_and_does_not_apply(cx: &mut TestAppContext) {
    let writes = TemplateWrites::default();
    let (_runtime, root, visual) = workspace(cx, exec_marker(), AppConfig::default(), writes);
    open_picker(visual, &root);
    let can_copy = visual.update(|_, cx| {
        root.read(cx)
            .ui
            .template_ui
            .read(cx)
            .target
            .as_ref()
            .is_some_and(|target| target.can_copy)
    });
    assert!(
        !can_copy,
        "a program pane cannot donate its session to a copied pane"
    );
    click(visual, "layout-template-builtin-builtin-columns-2");
    visual.update(|window, cx| {
        let template_ui = root.read(cx).ui.template_ui.clone();
        let template_ui = template_ui.read(cx);
        assert!(template_ui.open);
        assert_eq!(template_ui.feedback, None);
        assert_eq!(
            template_ui.text(cx, keys::LAYOUT_TEMPLATES_NEEDS_SESSION),
            "Only a terminal or connection pane can be copied into the panes this layout adds."
        );
        window.remove_window();
    });
}

#[gpui_kit::test]
fn saving_an_unnamed_split_uses_the_plural_untitled_name(cx: &mut TestAppContext) {
    let writes = TemplateWrites::default();
    let (_runtime, root, visual) =
        workspace(cx, CommandSpec::Shell, AppConfig::default(), writes.clone());
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, CommandSpec::Shell, &workspace.ui.locale, cx)
            });
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Right, window, cx);
            workspace.open_layout_templates(window, cx);
        });
        window.render_frame(cx);
    });
    visual.simulate_keystrokes("enter");
    let deadline = Instant::now() + Duration::from_secs(10);
    while visual.update(|_, cx| {
        root.read(cx)
            .factory
            .config
            .read(cx)
            .config()
            .panes
            .templates
            .is_empty()
    }) {
        assert!(Instant::now() < deadline, "the template save should finish");
        visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            let template_ui = workspace.ui.template_ui.read(cx);
            assert!(template_ui.open, "saving keeps the picker open to add more");
            assert!(!template_ui.saving);
            assert_eq!(
                template_ui
                    .input
                    .as_ref()
                    .expect("the name field stays open")
                    .read(cx)
                    .value(),
                ""
            );
            let saved = template_ui.saved_templates(cx);
            assert_eq!(saved.len(), 1);
            assert_eq!(saved[0].name, "Layout of 2 panes");
            assert!(saved[0].id.starts_with("saved-"));
        });
        window.remove_window();
    });
    let writes = writes.lock().expect("the test writer lock is not poisoned");
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].len(), 1);
}

#[gpui_kit::test]
fn deleting_a_saved_layout_removes_only_that_card(cx: &mut TestAppContext) {
    let writes = TemplateWrites::default();
    let config = with_templates(vec![
        config_template("saved-one", "One"),
        config_template("saved-two", "Two"),
    ]);
    let (_runtime, root, visual) = workspace(cx, CommandSpec::Shell, config, writes.clone());
    open_picker(visual, &root);
    click(visual, "layout-template-delete-saved-one");
    let deadline = Instant::now() + Duration::from_secs(10);
    while visual.update(|_, cx| {
        root.read(cx)
            .factory
            .config
            .read(cx)
            .config()
            .panes
            .templates
            .len()
            != 1
    }) {
        assert!(Instant::now() < deadline, "the delete should finish");
        visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            let saved = workspace.ui.template_ui.read(cx).saved_templates(cx);
            assert_eq!(saved.len(), 1);
            assert_eq!(saved[0].id, "saved-two");
            assert_eq!(saved[0].name, "Two");
        });
        window.remove_window();
    });
    assert_eq!(
        writes
            .lock()
            .expect("the test writer lock is not poisoned")
            .len(),
        1
    );
}

#[gpui_kit::test]
fn escape_and_cancel_restore_the_previous_focus(cx: &mut TestAppContext) {
    let writes = TemplateWrites::default();
    let (_runtime, root, visual) = workspace(cx, CommandSpec::Shell, AppConfig::default(), writes);
    let previous = visual.update(|window, cx| {
        let previous = cx.focus_handle();
        previous.focus(window, cx);
        root.update(cx, |workspace, cx| {
            workspace.open_layout_templates(window, cx);
        });
        window.render_frame(cx);
        previous
    });
    visual.simulate_keystrokes("escape");
    visual.update(|window, cx| {
        assert!(!root.read(cx).ui.template_ui.read(cx).open);
        assert!(previous.is_focused(window));
        root.update(cx, |workspace, cx| {
            workspace.open_layout_templates(window, cx);
        });
        window.render_frame(cx);
    });
    click(visual, CANCEL_BUTTON_ID);
    visual.update(|window, cx| {
        assert!(!root.read(cx).ui.template_ui.read(cx).open);
        assert!(previous.is_focused(window));
        window.remove_window();
    });
}

#[gpui_kit::test]
fn closing_the_target_tab_dismisses_the_picker(cx: &mut TestAppContext) {
    let writes = TemplateWrites::default();
    let (_runtime, root, visual) = workspace(cx, CommandSpec::Shell, AppConfig::default(), writes);
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, CommandSpec::Shell, &workspace.ui.locale, cx)
            });
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Right, window, cx);
            workspace.select_tab(1, window, cx);
            workspace.open_layout_templates(window, cx);
            workspace.close_tab(1, window, cx);
        });
        window.render_frame(cx);
    });
    visual.update(|window, cx| {
        assert!(!root.read(cx).ui.template_ui.read(cx).open);
        window.remove_window();
    });
}
