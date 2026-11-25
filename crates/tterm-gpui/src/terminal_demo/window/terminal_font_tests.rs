//! The shared config font reaches every pane without replacing its session.
//!
//! The harness opens a real window with local PTYs, so the assertions cover
//! the production factory, the committed [`ConfigState`] snapshot, the
//! TerminalFontChanged path and the terminal views together: a new pane
//! starts with the configured family and size, a committed change re-measures
//! every active and background pane, and a failed or equivalent save leaves
//! them alone.

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Context, TestAppContext, VisualTestContext};
use tokio::sync::mpsc::UnboundedReceiver;
use tterm_core::config::AppConfig;
use tterm_core::config::terminal_font_file::SavedTerminalFont;
use tterm_core::events::AppEvent;
use tterm_core::i18n::language::Language;
use tterm_terminal::grid::GridSize;
use tterm_terminal::snapshot::CursorShape;
use tterm_terminal::terminal::Terminal;
use tterm_terminal_view::view::layout::TerminalPadding;

use super::split_state::Edge;
use super::*;
use crate::app_window::background;
use crate::app_window::state::{SaveQueue, WindowGeometry};
use crate::config_state::{ConfigState, TerminalFontWriter};
use crate::locale::{Locale, LocaleGlobal};
use crate::terminal_demo::binding::SessionManager;
use crate::terminal_demo::command::CommandSpec;
use crate::terminal_demo::keymap::{KeymapRouter, encoder_config};
use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};

/// The family the tests save into the config.
const FAMILY: &str = "\"Fira Code\", monospace";
const SIZE: u16 = 20;

fn shell() -> CommandSpec {
    #[cfg(windows)]
    let (program, args) = ("cmd.exe", vec!["/d".into(), "/q".into()]);
    #[cfg(not(windows))]
    let (program, args) = ("/bin/sh", vec!["-i".into()]);
    CommandSpec::Program {
        program: program.into(),
        args,
    }
}

fn config() -> AppConfig {
    AppConfig {
        font_family: FAMILY.to_string(),
        font_size: SIZE,
        ..AppConfig::default()
    }
}

/// A writer that reports what the save asked for, as the real file reads back.
fn echoing_font() -> TerminalFontWriter {
    Arc::new(|family, size| Ok(SavedTerminalFont { family, size }))
}

fn failing_font() -> TerminalFontWriter {
    Arc::new(|_, _| Err("the config file is read-only".to_string()))
}

struct Harness<'a> {
    _runtime: GpuiRuntime,
    root: Entity<Prototype>,
    visual: &'a mut VisualTestContext,
    config: Entity<ConfigState>,
}

fn harness<'a>(cx: &'a mut TestAppContext) -> Harness<'a> {
    harness_with(cx, config(), echoing_font())
}

fn harness_with<'a>(
    cx: &'a mut TestAppContext,
    config: AppConfig,
    font_writer: TerminalFontWriter,
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
        cx.add_window_view(|_, cx| make_workspace(&runtime, events, config, font_writer, cx));
    let config = visual.update(|_, cx| root.read(cx).factory.config.clone());
    Harness {
        _runtime: runtime,
        root,
        visual,
        config,
    }
}

fn make_workspace(
    runtime: &GpuiRuntime,
    events: UnboundedReceiver<AppEvent>,
    config: AppConfig,
    font_writer: TerminalFontWriter,
    cx: &mut Context<Prototype>,
) -> Prototype {
    let keymap = Rc::new(KeymapRouter::new(&config.keymap, cfg!(target_os = "macos")));
    let locale = cx.new(|_| Locale::new(Language::En));
    LocaleGlobal::set(cx, locale.clone());
    let handle = runtime._runtime.handle();
    let queue = SaveQueue::start(handle, Arc::new(|_| Ok(())));
    let geometry = cx.new(|_| WindowGeometry::new(None, Default::default(), queue));
    let config = cx.new(|_| ConfigState::with_terminal_font_writer(config, handle, font_writer));
    Prototype::new(
        cx,
        PrototypeStartup {
            command: shell(),
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

impl Harness<'_> {
    /// Opens one more tab and activates it, as the tab strip's `+` does.
    fn open_tab(&mut self) {
        let root = self.root.clone();
        self.visual.update(|window, cx| {
            root.update(cx, |workspace, cx| workspace.new_tab(window, cx));
        });
    }

    fn draw(&mut self) {
        self.visual.update(|window, cx| window.render_frame(cx));
    }

    /// Saves a font through the settings path and waits for the commit.
    async fn save_font(&mut self, family: &str, size: u16) -> Result<(), String> {
        let family = family.to_string();
        let saved = self.visual.update(|_, cx| {
            self.config
                .update(cx, |state, cx| state.save_terminal_font(family, size, cx))
        });
        saved.await
    }

    fn font(&self, id: usize) -> (Vec<String>, f32) {
        self.root.read_with(self.visual, |workspace, cx| {
            let font = workspace
                .tabs
                .get(id)
                .expect("the tab")
                .view
                .read(cx)
                .font();
            (font.families.clone(), font.size)
        })
    }

    fn measurements(&self, id: usize) -> u64 {
        self.root.read_with(self.visual, |workspace, cx| {
            workspace
                .tabs
                .get(id)
                .expect("the tab")
                .view
                .read(cx)
                .stats()
                .font_measurements
        })
    }

    fn grid(&self, id: usize) -> GridSize {
        self.root
            .read_with(self.visual, |workspace, cx| {
                workspace
                    .tabs
                    .get(id)
                    .expect("the tab")
                    .view
                    .read(cx)
                    .grid_size()
            })
            .expect("a prepared grid")
    }

    fn model(&self, id: usize) -> Arc<Terminal> {
        self.root.read_with(self.visual, |workspace, cx| {
            workspace
                .tabs
                .get(id)
                .expect("the tab")
                .view
                .read(cx)
                .model()
        })
    }

    fn visible(&self, id: usize) -> bool {
        self.root.read_with(self.visual, |workspace, cx| {
            workspace
                .tabs
                .get(id)
                .expect("the tab")
                .view
                .read(cx)
                .is_visible()
        })
    }

    fn measured(&self, id: usize) -> bool {
        self.root.read_with(self.visual, |workspace, cx| {
            workspace
                .tabs
                .get(id)
                .expect("the tab")
                .view
                .read(cx)
                .cell_geometry()
                .is_some()
        })
    }

    fn padding(&self, id: usize) -> TerminalPadding {
        self.root.read_with(self.visual, |workspace, cx| {
            workspace
                .tabs
                .get(id)
                .expect("the tab")
                .view
                .read(cx)
                .padding()
        })
    }

    fn clipboard(&self, id: usize) -> (bool, bool) {
        self.root.read_with(self.visual, |workspace, cx| {
            let view = workspace.tabs.get(id).expect("the tab").view.read(cx);
            (view.copy_on_select(), view.right_click_paste())
        })
    }

    fn multiline_confirmation(&self, id: usize) -> bool {
        self.root.read_with(self.visual, |workspace, cx| {
            workspace
                .tabs
                .get(id)
                .expect("the tab")
                .paste_controller
                .as_ref()
                .expect("a live tab has a paste controller")
                .read(cx)
                .confirm_multiline()
        })
    }

    /// The shape a reset cursor returns to in a pane's live model.
    fn reset_cursor_shape(&self, id: usize) -> CursorShape {
        let model = self.model(id);
        model.write(b"\x1b[0 q");
        model.snapshot().cursor().shape
    }
}

fn menlo() -> (Vec<String>, f32) {
    (vec!["Menlo".to_string(), "monospace".to_string()], 24.0)
}

#[gpui_kit::test]
fn the_first_pane_draws_with_the_committed_config_font(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.draw();
    assert_eq!(
        h.font(1),
        (
            vec!["Fira Code".to_string(), "monospace".to_string()],
            f32::from(SIZE)
        )
    );
    h.root.read_with(h.visual, |workspace, cx| {
        let metrics = workspace
            .tabs
            .get(1)
            .expect("the tab")
            .view
            .read(cx)
            .cell_geometry()
            .expect("measured metrics");
        assert_eq!(metrics.font_size, f32::from(SIZE));
    });
}

#[gpui_kit::test]
async fn a_committed_font_reaches_every_pane_and_keeps_their_sessions(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.open_tab();
    let root = h.root.clone();
    h.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Bottom, window, cx);
        });
    });
    h.draw();

    let first_model = h.model(1);
    let second_model = h.model(2);
    let first_grid = h.grid(1);
    let second_grid = h.grid(2);
    let first_binding = h.root.read_with(h.visual, |workspace, _| {
        Rc::clone(
            workspace
                .tabs
                .get(1)
                .expect("the first tab")
                .binding
                .as_ref()
                .expect("the first binding"),
        )
    });

    assert_eq!(h.save_font("Menlo, monospace", 24).await, Ok(()));

    assert_eq!(h.font(1), menlo());
    assert_eq!(h.font(2), menlo());
    assert_ne!(h.grid(1), first_grid, "the first pane re-measured");
    assert_ne!(h.grid(2), second_grid, "the second pane re-measured");

    // The session that owned each pane is still the one the pane draws.
    assert!(Arc::ptr_eq(&h.model(1), &first_model));
    assert!(Arc::ptr_eq(&h.model(2), &second_model));
    let binding_now = h.root.read_with(h.visual, |workspace, _| {
        Rc::clone(
            workspace
                .tabs
                .get(1)
                .expect("the first tab")
                .binding
                .as_ref()
                .expect("the first binding"),
        )
    });
    assert!(Rc::ptr_eq(&binding_now, &first_binding));
}

#[gpui_kit::test]
async fn a_background_tab_takes_the_font_and_resizes_when_it_is_shown(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.open_tab();
    h.draw();
    let background = 1;
    assert!(!h.visible(background));
    let grid_before = h.grid(background);
    let model = h.model(background);

    assert_eq!(h.save_font("Menlo, monospace", 22).await, Ok(()));
    assert_eq!(h.font(background).1, 22.0);
    assert!(
        !h.measured(background),
        "a background tab measures when it is shown again"
    );
    assert_eq!(
        model.grid_size(),
        grid_before,
        "a hidden PTY is not resized"
    );

    let root = h.root.clone();
    h.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.select_tab(background, window, cx)
        });
    });
    h.draw();
    assert_ne!(h.grid(background), grid_before);
    assert_eq!(model.grid_size(), h.grid(background));
}

#[gpui_kit::test]
async fn spacing_only_commit_refits_the_live_grid_and_new_tabs(cx: &mut TestAppContext) {
    use tterm_core::config::terminal_font_file::TerminalPageSettings;

    let mut harness = harness(cx);
    harness.draw();
    let before = harness.grid(1);
    let model = harness.model(1);
    let save = harness.visual.update(|_, cx| {
        harness.config.update(cx, |state, cx| {
            let mut page = TerminalPageSettings::from(state.config());
            page.line_height = 1.75;
            page.letter_spacing = 4;
            state.save_terminal_page(page, cx)
        })
    });
    save.await
        .expect("isolated page writer accepts the spacing");
    harness.draw();
    let after = harness.grid(1);
    assert!(after.columns < before.columns && after.screen_lines < before.screen_lines);
    assert!(Arc::ptr_eq(&model, &harness.model(1)));
    assert_eq!(model.grid_size(), after);
    harness.open_tab();
    harness.draw();
    harness.root.read_with(harness.visual, |workspace, cx| {
        let font = workspace.tabs.active().view.read(cx).font();
        assert_eq!((font.line_height, font.letter_spacing), (1.75, 4.0));
    });
}

#[gpui_kit::test]
async fn an_equivalent_chain_spelling_does_not_remeasure(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.draw();
    let font = h.font(1);
    let measurements = h.measurements(1);

    // The commit publishes a different string that parses to the same chain.
    assert_eq!(h.save_font("Fira Code,  monospace", SIZE).await, Ok(()));
    assert_eq!(h.font(1), font);
    assert_eq!(h.measurements(1), measurements);
}

#[gpui_kit::test]
async fn a_committed_page_reaches_split_and_new_panes_with_cursor_and_padding(
    cx: &mut TestAppContext,
) {
    use tterm_core::config::terminal_font_file::TerminalPageSettings;

    let mut h = harness(cx);
    h.open_tab();
    h.draw();
    let root = h.root.clone();
    h.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Bottom, window, cx);
        });
    });
    h.draw();

    let save = h.visual.update(|_, cx| {
        h.config.update(cx, |state, cx| {
            let mut page = TerminalPageSettings::from(state.config());
            page.cursor_style = "bar".to_string();
            page.padding_left = 13;
            page.padding_right = 5;
            page.padding_bottom = 9;
            state.save_terminal_page(page, cx)
        })
    });
    save.await.expect("the committed page is accepted");
    h.draw();

    for id in [1, 2] {
        assert_eq!(
            h.padding(id),
            TerminalPadding::new(13, 5, 9),
            "an existing split pane takes the page padding"
        );
        assert_eq!(
            h.reset_cursor_shape(id),
            CursorShape::Beam,
            "the live model resets to the configured bar"
        );
    }

    // A pane opened after the save reads the same snapshot, not a stale one.
    h.open_tab();
    h.draw();
    assert_eq!(h.padding(3), TerminalPadding::new(13, 5, 9));
    assert_eq!(h.reset_cursor_shape(3), CursorShape::Beam);
}

#[gpui_kit::test]
async fn a_committed_clipboard_page_reaches_existing_split_and_new_panes(cx: &mut TestAppContext) {
    use tterm_core::config::terminal_font_file::TerminalPageSettings;

    let mut h = harness(cx);
    h.open_tab();
    h.draw();
    let root = h.root.clone();
    h.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Right, window, cx);
        });
    });
    h.draw();

    let save = h.visual.update(|_, cx| {
        h.config.update(cx, |state, cx| {
            let mut page = TerminalPageSettings::from(state.config());
            page.confirm_multiline_paste = false;
            page.copy_on_select = true;
            page.right_click_paste = true;
            state.save_terminal_page(page, cx)
        })
    });
    save.await
        .expect("the committed clipboard page is accepted");
    h.draw();

    for id in [1, 2] {
        assert_eq!(
            h.clipboard(id),
            (true, true),
            "an existing split pane takes the clipboard rules"
        );
        assert!(
            !h.multiline_confirmation(id),
            "the open pane stops asking for multiline pastes"
        );
    }

    // A new tab and a pane split after the save read the committed snapshot.
    h.open_tab();
    h.open_tab();
    let root = h.root.clone();
    h.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.select_tab(3, window, cx);
            workspace.split_tab(4, 3, Edge::Bottom, window, cx);
        });
    });
    h.draw();
    for id in [3, 4] {
        assert_eq!(h.clipboard(id), (true, true));
        assert!(!h.multiline_confirmation(id));
    }
}

#[gpui_kit::test]
async fn an_unsaved_or_failed_clipboard_page_leaves_every_pane_alone(cx: &mut TestAppContext) {
    use tterm_core::config::terminal_font_file::TerminalPageSettings;

    let initial = AppConfig {
        confirm_multiline_paste: true,
        copy_on_select: false,
        right_click_paste: false,
        ..config()
    };
    let mut h = harness_with(cx, initial, failing_font());
    h.draw();
    assert_eq!(h.clipboard(1), (false, false));
    assert!(h.multiline_confirmation(1));

    // Editing the draft only changes the local value; the pane keeps the
    // committed one until the page is saved.
    let draft = h.config.read_with(h.visual, |state, _| {
        let mut page = TerminalPageSettings::from(state.config());
        page.confirm_multiline_paste = false;
        page.copy_on_select = true;
        page.right_click_paste = true;
        page
    });
    h.draw();
    assert_eq!(h.clipboard(1), (false, false));
    assert!(h.multiline_confirmation(1));

    let save = h.visual.update(|_, cx| {
        h.config
            .update(cx, |state, cx| state.save_terminal_page(draft, cx))
    });
    assert_eq!(save.await, Err("the config file is read-only".to_string()));
    h.config.read_with(h.visual, |state, _| {
        assert!(state.config().confirm_multiline_paste);
        assert!(!state.config().copy_on_select);
        assert!(!state.config().right_click_paste);
    });
    h.draw();
    assert_eq!(h.clipboard(1), (false, false));
    assert!(h.multiline_confirmation(1));
}

#[gpui_kit::test]
async fn a_failed_font_save_leaves_every_pane_on_the_committed_font(cx: &mut TestAppContext) {
    let mut h = harness_with(cx, config(), failing_font());
    h.draw();
    let font = h.font(1);
    let measurements = h.measurements(1);

    assert_eq!(
        h.save_font("Menlo, monospace", 24).await,
        Err("the config file is read-only".to_string())
    );
    h.config.read_with(h.visual, |state, _| {
        assert_eq!(state.config().font_family, FAMILY);
        assert_eq!(state.config().font_size, SIZE);
    });
    h.draw();
    assert_eq!(h.font(1), font);
    assert_eq!(
        h.measurements(1),
        measurements,
        "a rejected save must not re-measure a pane"
    );
}
