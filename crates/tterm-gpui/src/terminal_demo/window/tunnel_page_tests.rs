//! The Port Forwarding route over two live split sessions.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{App, MouseButton, TestAppContext, VisualTestContext};
use tterm_core::config::AppConfig;
use tterm_core::workspace::tab_groups::{PaneNode, SplitOrientation};

use super::split_state::Edge;
use super::tunnel_page::{CLOSE_BUTTON_ID, OPEN_BUTTON_ID, TAB_ID};
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
        ConfigState::with_writers(
            config,
            handle,
            scrollback_writer,
            pane_templates_writer,
            crate::config_state::echo_local_shells_writer(),
        )
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
            theme: theme::default_theme(),
        },
        geometry,
    )
}

struct Harness<'a> {
    _runtime: GpuiRuntime,
    root: Entity<Prototype>,
    visual: &'a mut VisualTestContext,
    first: Rc<TabBinding>,
    second: Rc<TabBinding>,
    first_model: Arc<Terminal>,
    second_model: Arc<Terminal>,
}

fn harness<'a>(cx: &'a mut TestAppContext, config: AppConfig) -> Harness<'a> {
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
    let writes = Arc::new(Mutex::new(Vec::new()));
    let scrollback_writer: ScrollbackWriter = Arc::new(move |lines| {
        writes.lock().expect("scrollback writes").push(lines);
        Ok(())
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
            workspace.focus_pane(2, window, cx);
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
            [tab_binding(workspace, 1), tab_binding(workspace, 2)]
        })
    });
    let first_model = first.session().expect("first session").model();
    let second_model = second.session().expect("second session").model();
    Harness {
        _runtime: runtime,
        root,
        visual,
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

fn page_count(visual: &mut VisualTestContext) -> usize {
    visual.update(|window, cx| window.render_frame(cx));
    usize::from(visual.debug_bounds("tunnel-page").is_some())
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

/// The saved workspace with program-supplied tab titles blanked out. Titles
/// follow the shell's prompt and can change between two reads for reasons the
/// page does not control; everything else must stay byte-for-byte the same.
fn without_program_titles(
    mut snapshot: Option<tterm_core::session_restore::workspace::WorkspaceSnapshot>,
) -> Option<tterm_core::session_restore::workspace::WorkspaceSnapshot> {
    if let Some(snapshot) = &mut snapshot {
        for tab in &mut snapshot.tabs {
            tab.set_title(String::new());
        }
    }
    snapshot
}

#[gpui_kit::test]
fn titlebar_and_page_tab_open_one_route_and_close_returns_focus(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let first_model = Arc::clone(&harness.first_model);
    let second_model = Arc::clone(&harness.second_model);
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| workspace.focus_pane(1, window, cx));
    });
    send_marker(harness.visual, &first_model, "beforefirst");
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| workspace.focus_pane(2, window, cx));
    });
    send_marker(harness.visual, &second_model, "beforesecond");
    let before = settled_outputs(&[first_model.as_ref(), second_model.as_ref()]);
    let before_workspace = harness
        .visual
        .update(|_, cx| root.read(cx).workspace_snapshot(cx));

    click(harness.visual, OPEN_BUTTON_ID);
    assert_eq!(page_count(harness.visual), 1);
    assert!(harness.visual.debug_bounds(TAB_ID).is_some());
    assert!(harness.visual.debug_bounds("session-tab-1").is_some());
    assert!(harness.visual.debug_bounds("session-tab-2").is_some());
    harness.visual.update(|_window, cx| {
        let workspace = root.read(cx);
        assert!(workspace.tunnel_page_is_open());
        assert_eq!(workspace.visible_ids(), vec![1, 2]);
        assert_eq!(workspace.tabs.active_id(), 2);
        assert_eq!(workspace.splits.split_sizes(1, &[]), Some(vec![0.35, 0.65]));
        assert_sessions_unchanged(workspace, &harness.first, &harness.second, 2);
        assert_eq!(
            without_program_titles(workspace.workspace_snapshot(cx)),
            without_program_titles(before_workspace.clone())
        );
    });

    click(harness.visual, OPEN_BUTTON_ID);
    click(harness.visual, TAB_ID);
    assert_eq!(page_count(harness.visual), 1);
    assert_eq!(
        harness
            .visual
            .update(|_, cx| root.read(cx).tabs.active_id()),
        2
    );

    click(harness.visual, CLOSE_BUTTON_ID);
    assert_eq!(page_count(harness.visual), 0);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.tunnel_page_is_open());
        assert!(pane_focus(workspace, 2, cx).is_focused(window));
        assert_eq!(workspace.visible_ids(), vec![1, 2]);
        assert_eq!(workspace.splits.split_sizes(1, &[]), Some(vec![0.35, 0.65]));
        assert_sessions_unchanged(workspace, &harness.first, &harness.second, 2);
        assert_eq!(
            without_program_titles(workspace.workspace_snapshot(cx)),
            without_program_titles(before_workspace.clone())
        );
    });
    assert_eq!(
        settled_outputs(&[first_model.as_ref(), second_model.as_ref()]),
        before
    );

    click(harness.visual, OPEN_BUTTON_ID);
    click(harness.visual, CLOSE_BUTTON_ID);
    assert_eq!(page_count(harness.visual), 0);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(pane_focus(workspace, 2, cx).is_focused(window));
        assert_eq!(
            without_program_titles(workspace.workspace_snapshot(cx)),
            without_program_titles(before_workspace.clone())
        );
    });

    send_marker(harness.visual, &second_model, "afterpage");
    assert!(!output(&first_model).contains("afterpage"));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn page_keeps_background_ptys_running_and_never_accepts_their_input(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let first_model = Arc::clone(&harness.first_model);
    let second_model = Arc::clone(&harness.second_model);
    let background = Rc::clone(&harness.second);
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| workspace.focus_pane(1, window, cx));
    });
    send_marker(harness.visual, &first_model, "beforeinput");
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| workspace.focus_pane(2, window, cx));
    });

    click(harness.visual, OPEN_BUTTON_ID);
    assert_eq!(page_count(harness.visual), 1);
    harness.visual.simulate_input("leakprobe");
    harness.visual.update(|window, cx| window.render_frame(cx));
    std::thread::sleep(Duration::from_millis(250));
    assert!(!output(&first_model).contains("leakprobe"));
    assert!(!output(&second_model).contains("leakprobe"));

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
        .update(|_, cx| assert!(root.read(cx).tunnel_page_is_open()));

    click(harness.visual, CLOSE_BUTTON_ID);
    assert_eq!(page_count(harness.visual), 0);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(pane_focus(workspace, 2, cx).is_focused(window));
        assert_eq!(workspace.visible_ids(), vec![1, 2]);
    });
    send_marker(harness.visual, &second_model, "afterbackground");
    assert!(!output(&first_model).contains("afterbackground"));
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn selecting_a_session_tab_leaves_the_page_without_replacing_panes(cx: &mut TestAppContext) {
    let harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let before = harness
        .visual
        .update(|_, cx| root.read(cx).workspace_snapshot(cx));

    click(harness.visual, OPEN_BUTTON_ID);
    assert_eq!(page_count(harness.visual), 1);
    click(harness.visual, "session-tab-1");
    assert_eq!(page_count(harness.visual), 0);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert!(!workspace.tunnel_page_is_open());
        assert_eq!(workspace.tabs.active_id(), 1);
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
        assert_eq!(workspace.visible_ids(), vec![1, 2]);
        assert_eq!(workspace.splits.split_sizes(1, &[]), Some(vec![0.35, 0.65]));
        assert_sessions_unchanged(workspace, &harness.first, &harness.second, 1);
        let after = workspace.workspace_snapshot(cx).expect("snapshot");
        let before = before.expect("snapshot");
        assert_eq!(
            after
                .tabs
                .iter()
                .map(|tab| tab.id.as_str())
                .collect::<Vec<_>>(),
            before
                .tabs
                .iter()
                .map(|tab| tab.id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(after.groups.len(), before.groups.len());
        for (after, before) in after.groups.iter().zip(&before.groups) {
            assert_eq!(after.root, before.root);
            assert_eq!(after.zoomed_tab_id, before.zoomed_tab_id);
        }
    });

    click(harness.visual, OPEN_BUTTON_ID);
    assert_eq!(page_count(harness.visual), 1);
    click(harness.visual, CLOSE_BUTTON_ID);
    assert_eq!(page_count(harness.visual), 0);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.active_id(), 1);
        assert!(pane_focus(workspace, 1, cx).is_focused(window));
    });
    harness.visual.update(|window, _| window.remove_window());
}
