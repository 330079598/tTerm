//! The settings route over two live split sessions.

#[path = "shell_launcher_tests.rs"]
mod shell_launcher_tests;

mod appearance;
mod clipboard;
mod command_marks;
mod font;
mod layout;
mod local_shells;
mod page_save;
mod routes;
mod scrollback;
mod search;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{App, MouseButton, TestAppContext, VisualTestContext};
use tterm_core::config::AppConfig;
use tterm_core::config::local_shells::LocalShell;
use tterm_core::config::local_shells_file::SavedLocalShells;
use tterm_core::config::scrollback::resolve_scrollback_lines;
use tterm_core::config::terminal_font_file::TerminalPageSettings;
use tterm_core::i18n::keys;
use tterm_core::workspace::local_shells::types::DefaultShellSettings;
use tterm_core::workspace::tab_groups::{PaneNode, SplitOrientation};

use super::settings_page;
use super::settings_page::{
    SETTINGS_OPEN_BUTTON_ID, SETTINGS_PAGE_ID, SETTINGS_TAB_LOCAL_SHELLS_ID,
    SETTINGS_TAB_TERMINAL_ID, SettingsTab,
};
use super::split_state::Edge;
use super::*;
use crate::config_state::{
    LocalShellsWriter, PaneTemplatesWriter, ScrollbackWriter, TerminalFontWriter,
    TerminalPageWriter,
};
use crate::local_shells_setting::{
    ARGS_INPUT_ID, EDIT_PREFIX, ERROR_ID as LOCAL_SHELLS_ERROR_ID, NAME_INPUT_ID,
    PAGE_ADD_ID as LOCAL_SHELLS_ADD_ID, PAGE_SECTION_ID as LOCAL_SHELLS_SECTION_ID,
    SAVE_ID as LOCAL_SHELLS_SAVE_ID,
};
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

struct WorkspaceWriters {
    scrollback: ScrollbackWriter,
    local_shells: LocalShellsWriter,
    font: Option<TerminalFontWriter>,
    page: Option<TerminalPageWriter>,
}

fn make_workspace(
    runtime: &GpuiRuntime,
    events: UnboundedReceiver<AppEvent>,
    config: AppConfig,
    writers: WorkspaceWriters,
    cx: &mut Context<Prototype>,
) -> Prototype {
    let keymap = Rc::new(KeymapRouter::new(&config.keymap, cfg!(target_os = "macos")));
    let locale = cx.new(|_| Locale::new(Language::En));
    LocaleGlobal::set(cx, locale.clone());
    let handle = runtime._runtime.handle();
    let queue = state::SaveQueue::start(handle, Arc::new(|_| Ok(())));
    let geometry = cx.new(|_| WindowGeometry::new(None, Default::default(), queue));
    let pane_templates_writer: PaneTemplatesWriter = Arc::new(|_| Ok(()));
    let config = cx.new(|_| match writers.page {
        Some(writer) => ConfigState::with_terminal_page_writer(config, handle, writer),
        None => match writers.font {
            Some(writer) => ConfigState::with_terminal_font_writer(config, handle, writer),
            None => ConfigState::with_writers(
                config,
                handle,
                writers.scrollback,
                pane_templates_writer,
                writers.local_shells,
            ),
        },
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
    local_shell_writes: Arc<Mutex<Vec<LocalShell>>>,
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
    let local_shell_writes = Arc::new(Mutex::new(Vec::new()));
    let recorded_shells = Arc::clone(&local_shell_writes);
    let local_shells_writer: LocalShellsWriter = Arc::new(move |saved| {
        recorded_shells
            .lock()
            .expect("local shell writes")
            .push(saved.clone());
        Ok(SavedLocalShells {
            local_shells: vec![saved],
            default_shell: DefaultShellSettings::default(),
        })
    });
    harness_with_writers(
        cx,
        config,
        scrollback_writer,
        writes,
        local_shells_writer,
        local_shell_writes,
        None,
    )
}

fn harness_with_font_writer<'a>(
    cx: &'a mut TestAppContext,
    config: AppConfig,
    writer: TerminalFontWriter,
) -> Harness<'a> {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let local_shell_writes = Arc::new(Mutex::new(Vec::new()));
    let local_shells_writer: LocalShellsWriter = Arc::new(|saved| {
        Ok(SavedLocalShells {
            local_shells: vec![saved],
            default_shell: DefaultShellSettings::default(),
        })
    });
    harness_with_writers(
        cx,
        config,
        Arc::new(|_| Ok(())),
        writes,
        local_shells_writer,
        local_shell_writes,
        Some(writer),
    )
}

fn harness_with_writer<'a>(
    cx: &'a mut TestAppContext,
    config: AppConfig,
    scrollback_writer: ScrollbackWriter,
    writes: Arc<Mutex<Vec<u32>>>,
) -> Harness<'a> {
    let local_shell_writes = Arc::new(Mutex::new(Vec::new()));
    let recorded_shells = Arc::clone(&local_shell_writes);
    let local_shells_writer: LocalShellsWriter = Arc::new(move |saved| {
        recorded_shells
            .lock()
            .expect("local shell writes")
            .push(saved.clone());
        Ok(SavedLocalShells {
            local_shells: vec![saved],
            default_shell: DefaultShellSettings::default(),
        })
    });
    harness_with_writers(
        cx,
        config,
        scrollback_writer,
        writes,
        local_shells_writer,
        local_shell_writes,
        None,
    )
}

struct HarnessWriters {
    scrollback: ScrollbackWriter,
    writes: Arc<Mutex<Vec<u32>>>,
    local_shells: LocalShellsWriter,
    local_shell_writes: Arc<Mutex<Vec<LocalShell>>>,
    font: Option<TerminalFontWriter>,
    page: Option<TerminalPageWriter>,
}

fn harness_with_writers<'a>(
    cx: &'a mut TestAppContext,
    config: AppConfig,
    scrollback_writer: ScrollbackWriter,
    writes: Arc<Mutex<Vec<u32>>>,
    local_shells_writer: LocalShellsWriter,
    local_shell_writes: Arc<Mutex<Vec<LocalShell>>>,
    font_writer: Option<TerminalFontWriter>,
) -> Harness<'a> {
    harness_with_options(
        cx,
        config,
        HarnessWriters {
            scrollback: scrollback_writer,
            writes,
            local_shells: local_shells_writer,
            local_shell_writes,
            font: font_writer,
            page: None,
        },
    )
}

fn harness_with_page_writer<'a>(
    cx: &'a mut TestAppContext,
    config: AppConfig,
    writer: TerminalPageWriter,
) -> Harness<'a> {
    let local_shell_writes = Arc::new(Mutex::new(Vec::new()));
    let local_shells_writer: LocalShellsWriter = Arc::new(|saved| {
        Ok(SavedLocalShells {
            local_shells: vec![saved],
            default_shell: DefaultShellSettings::default(),
        })
    });
    harness_with_options(
        cx,
        config,
        HarnessWriters {
            scrollback: Arc::new(|_| Ok(())),
            writes: Arc::new(Mutex::new(Vec::new())),
            local_shells: local_shells_writer,
            local_shell_writes,
            font: None,
            page: Some(writer),
        },
    )
}

fn harness_with_options<'a>(
    cx: &'a mut TestAppContext,
    config: AppConfig,
    writers: HarnessWriters,
) -> Harness<'a> {
    let HarnessWriters {
        scrollback,
        writes,
        local_shells,
        local_shell_writes,
        font,
        page,
    } = writers;
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
    let (root, visual) = cx.add_window_view(|_, cx| {
        make_workspace(
            &runtime,
            events,
            config,
            WorkspaceWriters {
                scrollback,
                local_shells,
                font,
                page,
            },
            cx,
        )
    });
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
        local_shell_writes,
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
    click_id(visual, id);
}

fn click_id(visual: &mut VisualTestContext, id: &str) {
    // The debug-selector lookup requires a `'static` id; leaking one test
    // string per click keeps the helper usable with generated row ids.
    let id: &'static str = Box::leak(id.to_string().into_boxed_str());
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

fn page_font_family_focused(root: &Entity<Prototype>, window: &Window, cx: &App) -> bool {
    root.read(cx)
        .ui
        .settings_page
        .read(cx)
        .font_family_focus(cx)
        .is_some_and(|focus| focus.is_focused(window))
}

fn page_font_size_focused(root: &Entity<Prototype>, window: &Window, cx: &App) -> bool {
    root.read(cx)
        .ui
        .settings_page
        .read(cx)
        .font_size_focus(cx)
        .is_some_and(|focus| focus.is_focused(window))
}

fn page_font_line_height_focused(root: &Entity<Prototype>, window: &Window, cx: &App) -> bool {
    root.read(cx)
        .ui
        .settings_page
        .read(cx)
        .font_line_height_focus(cx)
        .is_some_and(|focus| focus.is_focused(window))
}

fn page_font_letter_spacing_focused(root: &Entity<Prototype>, window: &Window, cx: &App) -> bool {
    root.read(cx)
        .ui
        .settings_page
        .read(cx)
        .font_letter_spacing_focus(cx)
        .is_some_and(|focus| focus.is_focused(window))
}

fn wait_for_font_save(harness: &mut Harness<'_>, root: &Entity<Prototype>, size: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        harness.visual.update(|window, cx| window.render_frame(cx));
        let done = harness.visual.update(|_, cx| {
            let page = root.read(cx).ui.settings_page.read(cx);
            page.font_saved(cx) && page.font_values(cx).1 == size
        });
        if done {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the font save"
        );
        harness.visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_font_page_error(harness: &mut Harness<'_>, root: &Entity<Prototype>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        harness.visual.update(|window, cx| window.render_frame(cx));
        if harness.visual.update(|_, cx| {
            root.read(cx)
                .ui
                .settings_page
                .read(cx)
                .font_error(cx)
                .is_some()
        }) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the font save error"
        );
        harness.visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
}

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

fn replace_focused_text(harness: &mut Harness<'_>, text: &str) {
    press(harness.visual, &format!("{}-a", modifier()));
    harness.visual.simulate_input(text);
    harness.visual.update(|window, cx| window.render_frame(cx));
}

fn open_scrollback_from_search(harness: &mut Harness<'_>, root: &Entity<Prototype>) {
    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(harness, "scrollback");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.search_query(), "");
        assert!(
            page_scrollback_focused(root, window, cx),
            "opening the result should focus its editable value"
        );
    });
}

fn page_search_focused(root: &Entity<Prototype>, window: &Window, cx: &App) -> bool {
    root.read(cx)
        .ui
        .settings_page
        .read(cx)
        .search_focus(cx)
        .is_some_and(|focus| focus.is_focused(window))
}

fn page_scrollback_focused(root: &Entity<Prototype>, window: &Window, cx: &App) -> bool {
    root.read(cx)
        .ui
        .settings_page
        .read(cx)
        .scrollback_focus(cx)
        .is_some_and(|focus| focus.is_focused(window))
}

fn wait_for_write_attempts(writes: &Arc<Mutex<Vec<u32>>>, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if writes.lock().expect("scrollback write attempts").len() >= expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected} scrollback write attempt(s)"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_local_shells(harness: &mut Harness<'_>, expected: usize) {
    let config = harness.config.clone();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let saved = harness.visual.update(|_, cx| {
            let state = config.read(cx);
            state.config().local_shells.len() == expected && !state.is_save_pending()
        });
        if saved {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected} local shell(s)"
        );
        harness.visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_local_shell_writes(harness: &mut Harness<'_>, expected: usize) {
    let config = harness.config.clone();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let settled = harness
            .visual
            .update(|_, cx| !config.read(cx).is_save_pending());
        if settled
            && harness
                .local_shell_writes
                .lock()
                .expect("local shell writes")
                .len()
                >= expected
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected} local shell write(s)"
        );
        harness.visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_local_shells_error(harness: &mut Harness<'_>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        harness.visual.update(|window, cx| window.render_frame(cx));
        if harness.visual.debug_bounds(LOCAL_SHELLS_ERROR_ID).is_some() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the local shell error"
        );
        harness.visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
}
