//! The settings route over two live split sessions.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{App, MouseButton, TestAppContext, VisualTestContext};
use tterm_core::config::AppConfig;
use tterm_core::config::scrollback::resolve_scrollback_lines;
use tterm_core::workspace::tab_groups::{PaneNode, SplitOrientation};

use super::settings_page::{SETTINGS_OPEN_BUTTON_ID, SETTINGS_PAGE_ID};
use super::split_state::Edge;
use super::*;
use crate::config_state::{PaneTemplatesWriter, ScrollbackWriter};
use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};

fn test_shell() -> CommandSpec {
    #[cfg(windows)]
    let (program, args) = (
        "cmd.exe",
        vec![
            "/d".into(),
            "/q".into(),
            "/k".into(),
            "prompt".into(),
            "ROUTEREADY".into(),
        ],
    );
    #[cfg(not(windows))]
    let (program, args) = (
        "/usr/bin/env",
        vec![
            "PS1=ROUTEREADY".into(),
            "ENV=".into(),
            "/bin/sh".into(),
            "-i".into(),
        ],
    );
    CommandSpec::Program {
        program: program.into(),
        args,
    }
}

fn make_workspace(
    runtime: &GpuiRuntime,
    events: UnboundedReceiver<AppEvent>,
    config: AppConfig,
    scrollback_writer: ScrollbackWriter,
    cx: &mut Context<Prototype>,
) -> Prototype {
    let keymap = Rc::new(KeymapRouter::new(&config.keymap, cfg!(target_os = "macos")));
    let locale = cx.new(|_| Locale::new(Language::En));
    LocaleGlobal::set(cx, locale.clone());
    let handle = runtime._runtime.handle();
    let queue = state::SaveQueue::start(handle, Arc::new(|_| Ok(())));
    let geometry = cx.new(|_| WindowGeometry::new(None, Default::default(), queue));
    let pane_templates_writer: PaneTemplatesWriter = Arc::new(|_| Ok(()));
    let config = cx.new(|_| {
        ConfigState::with_writers(config, handle, scrollback_writer, pane_templates_writer)
    });
    Prototype::new(
        cx,
        PrototypeStartup {
            command: test_shell(),
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

struct Harness<'a> {
    _runtime: GpuiRuntime,
    root: Entity<Prototype>,
    visual: &'a mut VisualTestContext,
    config: Entity<ConfigState>,
    writes: Arc<Mutex<Vec<u32>>>,
    first: Rc<TabBinding>,
    second: Rc<TabBinding>,
    first_model: Arc<Terminal>,
    second_model: Arc<Terminal>,
}

fn harness<'a>(cx: &'a mut TestAppContext, config: AppConfig) -> Harness<'a> {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let scrollback_writer: ScrollbackWriter = Arc::new(move |lines| {
        recorded.lock().expect("scrollback writes").push(lines);
        Ok(())
    });
    harness_with_writer(cx, config, scrollback_writer, writes)
}

fn harness_with_writer<'a>(
    cx: &'a mut TestAppContext,
    config: AppConfig,
    scrollback_writer: ScrollbackWriter,
    writes: Arc<Mutex<Vec<u32>>>,
) -> Harness<'a> {
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
        cx.add_window_view(|_, cx| make_workspace(&runtime, events, config, scrollback_writer, cx));
    let [first, second] = visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, test_shell(), &workspace.ui.locale, cx)
            });
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Bottom, window, cx);
            assert!(workspace.splits.resize(1, &[], &[0.35, 0.65]));
            workspace.focus_pane(1, window, cx);
            assert_panes(workspace, &[1, 2], 1, window, cx);
            match workspace
                .splits
                .root(1)
                .expect("the two panes share a split")
            {
                PaneNode::Split { orientation, .. } => {
                    assert_eq!(*orientation, SplitOrientation::Vertical);
                }
                PaneNode::Pane { .. } => panic!("the panes should be split"),
            }
            assert_eq!(workspace.splits.split_sizes(1, &[]), Some(vec![0.35, 0.65]));
            [tab_binding(workspace, 1), tab_binding(workspace, 2)]
        })
    });
    let config = visual.update(|_, cx| root.read(cx).factory.config.clone());
    let first_model = first.session().expect("first session").model();
    let second_model = second.session().expect("second session").model();
    Harness {
        _runtime: runtime,
        root,
        visual,
        config,
        writes,
        first,
        second,
        first_model,
        second_model,
    }
}

fn tab_binding(workspace: &Prototype, id: usize) -> Rc<TabBinding> {
    Rc::clone(
        workspace
            .tabs
            .get(id)
            .expect("the tab exists")
            .binding
            .as_ref()
            .expect("the tab has a live binding"),
    )
}

fn pane_focus(workspace: &Prototype, id: usize, cx: &App) -> gpui_kit::FocusHandle {
    workspace
        .tabs
        .get(id)
        .expect("the tab exists")
        .view
        .read(cx)
        .focus_handle()
}

fn assert_panes(
    workspace: &Prototype,
    visible: &[usize],
    focused: usize,
    window: &Window,
    cx: &App,
) {
    assert_eq!(workspace.visible_ids(), visible);
    assert_eq!(workspace.tabs.active_id(), focused);
    for (id, tab) in workspace.tabs.ordered() {
        let view = tab.view.read(cx);
        assert_eq!(view.is_visible(), visible.contains(&id));
        assert_eq!(view.focus_handle().is_focused(window), id == focused);
    }
}

fn assert_route_open(
    workspace: &Prototype,
    visible: &[usize],
    focused: usize,
    window: &Window,
    cx: &App,
) {
    assert!(workspace.settings_is_open());
    assert_eq!(workspace.visible_ids(), visible);
    assert_eq!(workspace.tabs.active_id(), focused);
    for (id, tab) in workspace.tabs.ordered() {
        let view = tab.view.read(cx);
        assert!(
            !view.is_visible(),
            "pane {id} should be hidden under settings"
        );
        assert!(
            !view.focus_handle().is_focused(window),
            "pane {id} should not keep focus under settings"
        );
    }
}

fn assert_sessions_unchanged(
    workspace: &Prototype,
    first: &Rc<TabBinding>,
    second: &Rc<TabBinding>,
    active: usize,
) {
    assert_eq!(
        workspace
            .tabs
            .ordered()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(workspace.tabs.len(), 2);
    assert!(Rc::ptr_eq(first, &tab_binding(workspace, 1)));
    assert!(Rc::ptr_eq(second, &tab_binding(workspace, 2)));
    assert!(workspace.factory.sessions.tab("gpui-3").is_none());
    assert_eq!(workspace.tabs.active_id(), active);
}

fn output(model: &Terminal) -> String {
    model
        .snapshot()
        .rows()
        .iter()
        .flat_map(|row| row.cells())
        .map(|cell| cell.ch)
        .collect()
}

fn click(visual: &mut VisualTestContext, id: &'static str) {
    visual.update(|window, cx| window.render_frame(cx));
    let bounds = visual
        .debug_bounds(id)
        .unwrap_or_else(|| panic!("{id} should be laid out"));
    let center = bounds.center();
    visual.simulate_mouse_down(center, MouseButton::Left, Default::default());
    visual.simulate_mouse_up(center, MouseButton::Left, Default::default());
    visual.update(|window, cx| window.render_frame(cx));
}

fn settings_page_count(visual: &mut VisualTestContext) -> usize {
    visual.update(|window, cx| window.render_frame(cx));
    usize::from(visual.debug_bounds(SETTINGS_PAGE_ID).is_some())
}

fn settings_page_visible(visual: &mut VisualTestContext) -> bool {
    settings_page_count(visual) == 1
}

fn modifier() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    }
}

fn press(visual: &mut VisualTestContext, chord: &str) {
    visual.update(|window, cx| window.render_frame(cx));
    visual.simulate_keystrokes(chord);
    visual.update(|window, cx| window.render_frame(cx));
}

fn send_marker(visual: &mut VisualTestContext, model: &Terminal, marker: &str) {
    visual.update(|window, cx| window.render_frame(cx));
    let keys = format!(
        "e c h o space {} enter",
        marker
            .chars()
            .map(|letter| letter.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
    visual.simulate_keystrokes(&keys);
    wait_until(&format!("the focused PTY to receive {marker}"), || {
        output(model)
            .rsplit_once(marker)
            .is_some_and(|(_, tail)| tail.trim_end().ends_with("ROUTEREADY"))
    });
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn settled_outputs(models: &[&Terminal]) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut stable = models.iter().map(|model| output(model)).collect::<Vec<_>>();
    let mut stable_since = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(25));
        let current = models.iter().map(|model| output(model)).collect::<Vec<_>>();
        if current == stable {
            if stable_since.elapsed() >= Duration::from_millis(250) {
                return current;
            }
        } else {
            stable = current;
            stable_since = Instant::now();
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for Terminal output to settle"
        );
    }
}

fn wait_for_save(harness: &mut Harness<'_>, expected: u32) {
    let config = harness.config.clone();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let saved = harness.visual.update(|_, cx| {
            let state = config.read(cx);
            state.scrollback_lines() == expected && !state.is_save_pending()
        });
        if saved
            && harness.first_model.scrollback() == expected as usize
            && harness.second_model.scrollback() == expected as usize
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out saving scrollback {expected}"
        );
        harness.visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[gpui_kit::test]
fn titlebar_click_opens_one_route_and_back_restores_the_exact_pane(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let first_model = Arc::clone(&harness.first_model);
    let second_model = Arc::clone(&harness.second_model);
    send_marker(harness.visual, &first_model, "beforefirst");
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| workspace.focus_pane(2, window, cx));
    });
    send_marker(harness.visual, &second_model, "beforesecond");
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.focus_pane(1, window, cx);
            assert_panes(workspace, &[1, 2], 1, window, cx);
        });
    });
    let before = settled_outputs(&[first_model.as_ref(), second_model.as_ref()]);
    assert!(before[0].contains("beforefirst"));
    assert!(before[1].contains("beforesecond"));

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    let first = Rc::clone(&harness.first);
    let second = Rc::clone(&harness.second);
    assert_eq!(settings_page_count(harness.visual), 1);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 1, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 1);
        assert!(workspace.factory.sessions.tab("gpui-1").is_some());
        assert!(Rc::ptr_eq(&first, &tab_binding(workspace, 1)));
        assert!(Rc::ptr_eq(&second, &tab_binding(workspace, 2)));
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 1, window, cx);
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
        assert_sessions_unchanged(workspace, &first, &second, 1);
        assert_eq!(workspace.splits.split_sizes(1, &[]), Some(vec![0.35, 0.65]));
        assert_eq!(output(&first_model), before[0]);
        assert_eq!(output(&second_model), before[1]);
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert_eq!(settings_page_count(harness.visual), 1);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 1, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });

    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 1, window, cx);
        let focus = pane_focus(workspace, 1, cx);
        assert!(focus.is_focused(window));
        assert_sessions_unchanged(workspace, &first, &second, 1);
        assert_eq!(workspace.splits.split_sizes(1, &[]), Some(vec![0.35, 0.65]));
        assert_eq!(output(&first_model), before[0]);
        assert_eq!(output(&second_model), before[1]);
    });

    send_marker(harness.visual, &first_model, "backtopane");
    assert!(!output(&second_model).contains("backtopane"));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn keymap_opens_the_singleton_route_and_escape_returns_input(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let second_model = Arc::clone(&harness.second_model);
    let first_model = Arc::clone(&harness.first_model);
    let first = Rc::clone(&harness.first);
    let second = Rc::clone(&harness.second);
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.focus_pane(2, window, cx);
            assert_panes(workspace, &[1, 2], 2, window, cx);
        });
    });

    press(harness.visual, &format!("{}-,", modifier()));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 2, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 2);
    });
    assert_eq!(settings_page_count(harness.visual), 1);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert!(!settings_page_visible(harness.visual));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert_eq!(settings_page_count(harness.visual), 1);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 2, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 2);
    });

    press(harness.visual, &format!("{}-,", modifier()));
    assert_eq!(settings_page_count(harness.visual), 1);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 2, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 2);
    });

    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 2, window, cx);
        assert!(pane_focus(workspace, 2, cx).is_focused(window));
        assert_sessions_unchanged(workspace, &first, &second, 2);
    });

    send_marker(harness.visual, &second_model, "keymapreturn");
    assert!(!output(&first_model).contains("keymapreturn"));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn remapped_settings_key_replaces_the_default_chord(cx: &mut TestAppContext) {
    let mut config = AppConfig::default();
    config
        .keymap
        .bindings
        .insert("settings.open".into(), Some(vec!["mod+alt+p".into()]));
    let harness = harness(cx, config);
    let root = harness.root.clone();
    let first_model = Arc::clone(&harness.first_model);
    let first = Rc::clone(&harness.first);
    let second = Rc::clone(&harness.second);

    press(harness.visual, &format!("{}-,", modifier()));
    assert!(!settings_page_visible(harness.visual));
    harness
        .visual
        .update(|_, cx| assert!(!root.read(cx).settings_is_open()));

    let remapped = format!("{}-alt-p", modifier());
    press(harness.visual, &remapped);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 1, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });
    assert_eq!(settings_page_count(harness.visual), 1);

    press(harness.visual, &format!("{}-,", modifier()));
    harness
        .visual
        .update(|_, cx| assert!(root.read(cx).settings_is_open()));

    press(harness.visual, &remapped);
    assert_eq!(settings_page_count(harness.visual), 1);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 1, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });

    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 1, window, cx);
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    press(harness.visual, &remapped);
    assert_eq!(settings_page_count(harness.visual), 1);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 1, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });
    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    send_marker(harness.visual, &first_model, "remapreturn");
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn settings_input_does_not_leak_and_background_output_keeps_running(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let first_model = Arc::clone(&harness.first_model);
    let second_model = Arc::clone(&harness.second_model);
    send_marker(harness.visual, &first_model, "beforeinput");
    let before = settled_outputs(&[first_model.as_ref()])[0].clone();
    let background = Rc::clone(&harness.second);
    let first = Rc::clone(&harness.first);
    let second = Rc::clone(&harness.second);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert!(settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_route_open(workspace, &[1, 2], 1, window, cx);
        assert!(!pane_focus(workspace, 1, cx).is_focused(window));
        assert!(!pane_focus(workspace, 2, cx).is_focused(window));
    });

    harness.visual.simulate_input("leakprobe");
    harness.visual.update(|window, cx| window.render_frame(cx));
    std::thread::sleep(Duration::from_millis(250));
    assert!(!output(&first_model).contains("leakprobe"));
    assert!(!output(&second_model).contains("leakprobe"));
    harness.visual.update(|_, cx| {
        assert_eq!(
            root.read(cx).ui.settings_page.read(cx).search_query(),
            "leakprobe"
        );
    });

    let probe = if cfg!(windows) {
        "echo bgprobe\r\n"
    } else {
        "echo bgprobe\n"
    };
    background
        .session()
        .expect("the background session")
        .write_input(probe.as_bytes())
        .expect("the background PTY accepts input");
    wait_until("the hidden background session to print bgprobe", || {
        output(&second_model).contains("bgprobe")
    });
    assert!(!output(&first_model).contains("bgprobe"));
    harness
        .visual
        .update(|_, cx| assert!(root.read(cx).settings_is_open()));

    press(harness.visual, "escape");
    assert!(settings_page_visible(harness.visual));
    harness.visual.update(|_, cx| {
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
    });
    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 1, window, cx);
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
        assert_eq!(output(&first_model), before);
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });

    send_marker(harness.visual, &first_model, "afterbackground");
    assert!(output(&first_model).contains("afterbackground"));
    assert!(!output(&second_model).contains("afterbackground"));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn edited_scrollback_survives_reopening_and_stays_applied(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let first_model = Arc::clone(&harness.first_model);
    let writes = Arc::clone(&harness.writes);
    let config = harness.config.clone();
    let first = Rc::clone(&harness.first);
    let second = Rc::clone(&harness.second);

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness
        .visual
        .update(|_, cx| assert!(root.read(cx).settings_is_open()));

    // A value outside the field's range stays a draft: pressing Save must not
    // write it or shrink the live buffers.
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.set_settings_scrollback_value("20000001", window, cx);
        });
    });
    click(
        harness.visual,
        super::settings_page::SETTINGS_SCROLLBACK_SAVE_ID,
    );
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(root.read(cx).settings_is_open());
        assert_eq!(config.read(cx).scrollback_lines(), 10_000);
        assert!(
            root.read(cx)
                .ui
                .settings_page
                .read(cx)
                .scrollback_value(cx)
                .contains("20000001")
        );
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SCROLLBACK_ERROR_ID)
            .is_some()
    );

    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.set_settings_scrollback_value("400", window, cx);
        });
    });
    click(
        harness.visual,
        super::settings_page::SETTINGS_SCROLLBACK_SAVE_ID,
    );
    wait_for_save(&mut harness, 400);
    harness.visual.update(|_, cx| {
        assert!(root.read(cx).settings_is_open());
        assert_eq!(config.read(cx).scrollback_lines(), 400);
    });
    assert_eq!(
        first_model.scrollback(),
        resolve_scrollback_lines(Some(400.0)) as usize
    );

    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.set_settings_scrollback_value("800", window, cx);
        });
    });
    click(
        harness.visual,
        super::settings_page::SETTINGS_SCROLLBACK_SAVE_ID,
    );
    wait_for_save(&mut harness, 800);
    harness
        .visual
        .update(|_, cx| assert_eq!(config.read(cx).scrollback_lines(), 800));
    assert_eq!(
        first_model.scrollback(),
        resolve_scrollback_lines(Some(800.0)) as usize
    );

    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 1, window, cx);
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });

    press(harness.visual, &format!("{}-,", modifier()));
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(root.read(cx).settings_is_open());
        assert_eq!(
            root.read(cx).ui.settings_page.read(cx).scrollback_value(cx),
            "800"
        );
    });
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.set_settings_scrollback_value("1200", window, cx);
        });
    });
    click(
        harness.visual,
        super::settings_page::SETTINGS_SCROLLBACK_SAVE_ID,
    );
    wait_for_save(&mut harness, 1200);
    harness.visual.update(|_, cx| {
        assert!(root.read(cx).settings_is_open());
        assert_eq!(config.read(cx).scrollback_lines(), 1200);
    });
    assert_eq!(
        first_model.scrollback(),
        resolve_scrollback_lines(Some(1200.0)) as usize
    );

    press(harness.visual, "escape");
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.settings_is_open());
        assert_panes(workspace, &[1, 2], 1, window, cx);
        assert_sessions_unchanged(workspace, &first, &second, 1);
    });

    assert_eq!(
        *writes.lock().expect("scrollback writes"),
        vec![400, 800, 1200]
    );
    send_marker(harness.visual, &first_model, "aftersetting");
    harness.visual.update(|window, _| window.remove_window());
}

fn page_bounds(
    visual: &mut VisualTestContext,
    id: &'static str,
) -> gpui_kit::Bounds<gpui_kit::Pixels> {
    visual.update(|window, cx| window.render_frame(cx));
    visual
        .debug_bounds(id)
        .unwrap_or_else(|| panic!("{id} should be laid out"))
}

fn assert_page_box(
    visual: &mut VisualTestContext,
    id: &'static str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    let bounds = page_bounds(visual, id);
    let actual = (
        bounds.origin.x.as_f32(),
        bounds.origin.y.as_f32(),
        bounds.size.width.as_f32(),
        bounds.size.height.as_f32(),
    );
    assert!(
        (actual.0 - x).abs() <= 1.0
            && (actual.1 - y).abs() <= 1.0
            && (actual.2 - width).abs() <= 1.0
            && (actual.3 - height).abs() <= 1.0,
        "{id} is {actual:?}, expected ({x}, {y}, {width}, {height})"
    );
}

fn wait_for_settings_error(visual: &mut VisualTestContext) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        visual.update(|window, cx| window.render_frame(cx));
        if visual
            .debug_bounds(super::settings_page::SETTINGS_SCROLLBACK_ERROR_ID)
            .is_some()
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the settings save error"
        );
        visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn type_search(harness: &mut Harness<'_>, text: &str) {
    harness.visual.simulate_input(text);
    harness.visual.update(|window, cx| window.render_frame(cx));
}

#[gpui_kit::test]
fn search_finds_the_scrollback_setting_and_opens_it(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    assert!(settings_page_visible(harness.visual));

    type_search(&mut harness, "scrollback");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_query(), "scrollback");
        assert_eq!(page.search_result_count(cx), 1);
        assert_eq!(page.result_titles(cx), vec!["Scrollback Lines".to_string()]);
        window.render_frame(cx);
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_RESULT_0_ID)
            .is_some()
    );
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_EMPTY_ID)
            .is_none()
    );

    press(harness.visual, "down");
    press(harness.visual, "up");
    harness
        .visual
        .update(|_, cx| assert_eq!(root.read(cx).ui.settings_page.read(cx).active_index(), 0));

    press(harness.visual, "enter");
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_RESULTS_ID)
            .is_none()
    );
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(root.read(cx).settings_is_open());
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_value(cx), "");
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_RESULT_0_ID)
            .is_none()
    );

    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn search_reports_no_match_and_clears_before_closing(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    type_search(&mut harness, "zzz");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_result_count(cx), 0);
        assert_eq!(
            page.empty_message(cx),
            "No settings match \u{201c}zzz\u{201d}"
        );
        window.render_frame(cx);
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_EMPTY_ID)
            .is_some()
    );
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_RESULT_0_ID)
            .is_none()
    );

    click(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_CLEAR_ID,
    );
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_EMPTY_ID)
            .is_none()
    );
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_PANEL_ID)
            .is_some()
    );

    type_search(&mut harness, "scrollback");
    press(harness.visual, "escape");
    assert!(settings_page_visible(harness.visual));
    harness
        .visual
        .update(|_, cx| assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), ""));
    press(harness.visual, "escape");
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn search_accepts_chinese_and_mouse_selection(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    harness.visual.update(|_, cx| {
        let locale = root.read(cx).ui.locale.clone();
        locale.update(cx, |locale, cx| locale.set_language(Language::Zh, cx));
    });

    type_search(&mut harness, "回滚");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_query(), "回滚");
        assert_eq!(page.result_titles(cx), vec!["回滚行数".to_string()]);
        window.render_frame(cx);
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_RESULT_0_ID)
            .is_some()
    );

    click(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_RESULT_0_ID,
    );
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
        assert!(root.read(cx).settings_is_open());
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_PANEL_ID)
            .is_some()
    );

    harness.visual.update(|_, cx| {
        let locale = root.read(cx).ui.locale.clone();
        locale.update(cx, |locale, cx| locale.set_language(Language::En, cx));
    });
    type_search(&mut harness, "scrollback");
    click(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_RESULT_0_ID,
    );
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_PANEL_ID)
            .is_some()
    );

    click(
        harness.visual,
        super::settings_page::SETTINGS_OPEN_BUTTON_ID,
    );
    assert!(!settings_page_visible(harness.visual));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn page_layout_matches_the_reference_metrics(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(800.0), gpui_kit::px(636.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    // The 36px window title bar sits above the 800x600 settings content area.
    assert_page_box(harness.visual, SETTINGS_PAGE_ID, 0.0, 36.0, 800.0, 600.0);
    assert_page_box(
        harness.visual,
        super::settings_page::SETTINGS_HEADER_ID,
        0.0,
        36.0,
        800.0,
        73.0,
    );
    assert_page_box(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_FIELD_ID,
        496.0,
        60.0,
        256.0,
        32.0,
    );
    assert_page_box(
        harness.visual,
        super::settings_page::SETTINGS_SIDEBAR_ID,
        0.0,
        109.0,
        192.0,
        527.0,
    );
    assert_page_box(
        harness.visual,
        super::settings_page::SETTINGS_TAB_TERMINAL_ID,
        12.0,
        139.0,
        159.0,
        32.0,
    );
    assert_page_box(
        harness.visual,
        super::settings_page::SETTINGS_PANEL_ID,
        192.0,
        109.0,
        608.0,
        527.0,
    );
    assert_page_box(
        harness.visual,
        super::settings_page::SETTINGS_SCROLLBACK_SECTION_ID,
        216.0,
        133.0,
        536.0,
        111.0,
    );
    assert_page_box(
        harness.visual,
        super::settings_page::SETTINGS_SCROLLBACK_INPUT_ID,
        216.0,
        155.0,
        536.0,
        36.0,
    );

    if std::env::var_os("TTERM_GPUI_WRITE_SETTINGS_LAYOUT").is_some() {
        write_layout_json(harness.visual, "layout.json", &LAYOUT_IDS);
    }

    type_search(&mut harness, "scrollback");
    assert_page_box(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_RESULT_0_ID,
        216.0,
        154.0,
        560.0,
        40.0,
    );
    if std::env::var_os("TTERM_GPUI_WRITE_SETTINGS_LAYOUT").is_some() {
        write_layout_json(harness.visual, "layout-search.json", &SEARCH_LAYOUT_IDS);
    }

    press(harness.visual, "escape");
    type_search(&mut harness, "zzz");
    if std::env::var_os("TTERM_GPUI_WRITE_SETTINGS_LAYOUT").is_some() {
        write_layout_json(harness.visual, "layout-empty.json", &EMPTY_LAYOUT_IDS);
    }

    harness.visual.update(|window, _| window.remove_window());
}

const LAYOUT_IDS: [&str; 10] = [
    SETTINGS_PAGE_ID,
    super::settings_page::SETTINGS_HEADER_ID,
    super::settings_page::SETTINGS_TITLE_ID,
    super::settings_page::SETTINGS_SEARCH_FIELD_ID,
    super::settings_page::SETTINGS_SIDEBAR_ID,
    super::settings_page::SETTINGS_TAB_TERMINAL_ID,
    super::settings_page::SETTINGS_PANEL_ID,
    super::settings_page::SETTINGS_SCROLLBACK_SECTION_ID,
    super::settings_page::SETTINGS_SCROLLBACK_INPUT_ID,
    super::settings_page::SETTINGS_SCROLLBACK_SAVE_ID,
];

const SEARCH_LAYOUT_IDS: [&str; 6] = [
    SETTINGS_PAGE_ID,
    super::settings_page::SETTINGS_SEARCH_FIELD_ID,
    super::settings_page::SETTINGS_SEARCH_RESULTS_ID,
    super::settings_page::SETTINGS_SEARCH_COUNT_ID,
    super::settings_page::SETTINGS_SEARCH_RESULT_0_ID,
    super::settings_page::SETTINGS_SIDEBAR_ID,
];

const EMPTY_LAYOUT_IDS: [&str; 4] = [
    SETTINGS_PAGE_ID,
    super::settings_page::SETTINGS_SEARCH_FIELD_ID,
    super::settings_page::SETTINGS_SEARCH_EMPTY_ID,
    super::settings_page::SETTINGS_SIDEBAR_ID,
];

fn write_layout_json(visual: &mut VisualTestContext, file: &str, ids: &[&'static str]) {
    use std::path::Path;

    fn element(visual: &mut VisualTestContext, id: &'static str) -> serde_json::Value {
        let bounds = page_bounds(visual, id);
        serde_json::json!({
            "x": bounds.origin.x.as_f32(),
            "y": bounds.origin.y.as_f32() - 36.0,
            "width": bounds.size.width.as_f32(),
            "height": bounds.size.height.as_f32(),
        })
    }

    let elements: serde_json::Map<String, serde_json::Value> = ids
        .iter()
        .map(|id| (id.to_string(), element(visual, id)))
        .collect();
    let layout = serde_json::json!({
        "version": 1,
        "viewport": { "width": 800, "height": 600, "deviceScaleFactor": 2 },
        "titleBarHeight": 36,
        "coordinateSpace": "settings content, origin at the title bar's bottom edge",
        "elements": elements,
    });
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = root.join("target/parity/settings-gpui");
    std::fs::create_dir_all(&directory).expect("the layout directory is writable");
    std::fs::write(
        directory.join(file),
        serde_json::to_string_pretty(&layout).expect("the layout serializes") + "\n",
    )
    .expect("the layout is written");
    println!("settings layout: {}", directory.join(file).display());
}

#[gpui_kit::test]
fn a_failed_save_keeps_the_page_open_and_the_running_capacity(cx: &mut TestAppContext) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer: ScrollbackWriter = Arc::new(|_| Err("the config disk is full".to_string()));
    let harness = harness_with_writer(cx, AppConfig::default(), writer, writes);
    let root = harness.root.clone();
    let config = harness.config.clone();
    let first_model = Arc::clone(&harness.first_model);
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.set_settings_scrollback_value("500", window, cx);
        });
    });
    click(
        harness.visual,
        super::settings_page::SETTINGS_SCROLLBACK_SAVE_ID,
    );
    wait_for_settings_error(harness.visual);
    assert!(settings_page_visible(harness.visual));
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(root.read(cx).settings_is_open());
        assert_eq!(config.read(cx).scrollback_lines(), 10_000);
        assert_eq!(
            first_model.scrollback(),
            resolve_scrollback_lines(Some(10_000.0)) as usize
        );
        assert_eq!(
            root.read(cx).ui.settings_page.read(cx).scrollback_value(cx),
            "500"
        );
    });
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn arrow_keys_move_between_the_page_and_its_setting(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    type_search(&mut harness, "e");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_result_count(cx), 2);
        assert_eq!(page.active_index(), 0);
        window.render_frame(cx);
    });

    press(harness.visual, "down");
    harness
        .visual
        .update(|_, cx| assert_eq!(root.read(cx).ui.settings_page.read(cx).active_index(), 1));
    press(harness.visual, "up");
    harness
        .visual
        .update(|_, cx| assert_eq!(root.read(cx).ui.settings_page.read(cx).active_index(), 0));
    press(harness.visual, "down");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(root.read(cx).ui.settings_page.read(cx).search_query(), "");
        assert!(root.read(cx).settings_is_open());
    });
    assert!(
        harness
            .visual
            .debug_bounds(super::settings_page::SETTINGS_SEARCH_RESULT_0_ID)
            .is_none()
    );

    harness.visual.update(|window, _| window.remove_window());
}
