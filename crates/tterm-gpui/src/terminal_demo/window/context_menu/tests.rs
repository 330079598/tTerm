//! The pane Copy/Paste menu, exercised through real right-clicks and keys.
//!
//! The menu is the production `ContextMenu` entity, so these tests cover the
//! gesture that opens it, keyboard dismissal and focus restoration, and the
//! two actions. Paste keeps its gate: nothing reaches the session until the
//! gate delivers approved text.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{
    AppContext as _, ClipboardItem, Context, Entity, InteractiveElement, IntoElement, Modifiers,
    MouseButton, ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window, div,
    point, px,
};
use tterm_core::i18n::language::Language;
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::context_menu::{ContextMenuHandler, ContextMenuRequest};
use tterm_terminal_view::view::paste::PasteRequest;
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};
use tterm_ui::tokens::palette::DARK;

use super::{PaneContextMenu, actions};
use crate::locale::{Locale, LocaleGlobal};

type GateSlot = Rc<RefCell<Option<PasteRequest>>>;

/// One pane, its menu and the gate every paste request reaches.
struct MenuHarness {
    view: Entity<TerminalView>,
    menu: Entity<PaneContextMenu>,
    model: Arc<Terminal>,
    bytes: Rc<RefCell<Vec<u8>>>,
    gate: GateSlot,
}

impl MenuHarness {
    fn is_open(&self, cx: &gpui_kit::App) -> bool {
        self.menu.read(cx).is_open(cx)
    }

    fn view_focused(&self, window: &Window, cx: &gpui_kit::App) -> bool {
        self.view.read(cx).focus_handle().is_focused(window)
    }
}

impl Render for MenuHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "context-menu-harness".into())
            .relative()
            .size_full()
            .child(self.view.clone())
            .child(self.menu.clone())
    }
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        tterm_ui::init(cx);
        tterm_terminal_view::view::bind_keys(cx);
    });
}

fn model() -> Arc<Terminal> {
    let model = Arc::new(Terminal::new(
        Config::default(),
        GridSize::new(80, 24),
        CellSize::new(8, 16),
    ));
    model.write(b"hello world\r\n");
    model
}

fn view_config(
    bytes: Rc<RefCell<Vec<u8>>>,
    gate: GateSlot,
    right_click_paste: bool,
    context_menu: ContextMenuHandler,
) -> TerminalViewConfig {
    TerminalViewConfig {
        input: Some(Rc::new(move |input| {
            bytes.borrow_mut().extend_from_slice(input)
        })),
        right_click_paste,
        context_menu: Some(context_menu),
        paste_gate: Some(Rc::new(move |request: PasteRequest, _, _| {
            *gate.borrow_mut() = Some(request);
            true
        })),
        ..TerminalViewConfig::default()
    }
}

fn harness(
    cx: &mut TestAppContext,
    right_click_paste: bool,
) -> (Entity<MenuHarness>, VisualTestContext) {
    init(cx);
    let model = model();
    let bytes: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let gate: GateSlot = Rc::new(RefCell::new(None));
    let model_for_view = Arc::clone(&model);
    let bytes_for_view = Rc::clone(&bytes);
    let gate_for_view = Rc::clone(&gate);
    let (window, root) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let locale = cx.new(|_| Locale::new(Language::En));
            LocaleGlobal::set(cx, locale);
            let menu = cx.new(|_| PaneContextMenu::new(DARK));
            let config = view_config(
                bytes_for_view,
                gate_for_view,
                right_click_paste,
                super::view_handler(menu.clone()),
            );
            let view = cx.new(|cx| TerminalView::new(cx, model_for_view, config));
            window.focus(&view.read(cx).focus_handle(), cx);
            cx.new(|_| MenuHarness {
                view,
                menu,
                model,
                bytes,
                gate,
            })
        })
        .expect("open the context-menu test window")
    });
    (root, VisualTestContext::from_window(window, cx))
}

fn redraw(visual: &mut VisualTestContext) {
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

fn pane_center(visual: &mut VisualTestContext) -> gpui_kit::Point<gpui_kit::Pixels> {
    visual
        .debug_bounds("context-menu-harness")
        .expect("the harness is laid out")
        .center()
}

fn shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..Default::default()
    }
}

fn open_menu(visual: &mut VisualTestContext, modifiers: Modifiers) {
    let position = pane_center(visual);
    visual.simulate_mouse_down(position, MouseButton::Right, modifiers);
    redraw(visual);
}

fn is_open(root: &Entity<MenuHarness>, visual: &mut VisualTestContext) -> bool {
    visual.read(|cx| root.read(cx).is_open(cx))
}

fn clip(visual: &mut VisualTestContext) -> Option<String> {
    visual.read(|cx| cx.read_from_clipboard().and_then(|item| item.text()))
}

#[gpui_kit::test]
fn shift_right_click_opens_the_real_menu_and_escape_restores_the_pane_focus(
    cx: &mut TestAppContext,
) {
    let (root, mut visual) = harness(cx, false);
    redraw(&mut visual);
    assert!(!is_open(&root, &mut visual));
    open_menu(&mut visual, shift());
    assert!(is_open(&root, &mut visual), "Shift opens the menu");
    visual.update(|window, cx| {
        assert!(root.read(cx).menu.read(cx).is_open(cx));
        assert!(
            !root.read(cx).view_focused(window, cx),
            "the menu takes focus while it is open"
        );
    });

    visual.simulate_keystrokes("escape");
    redraw(&mut visual);
    assert!(!is_open(&root, &mut visual), "Escape dismisses the menu");
    visual.update(|window, cx| {
        assert!(
            root.read(cx).view_focused(window, cx),
            "focus returns to the pane the menu came from"
        );
    });
}

#[gpui_kit::test]
fn a_click_outside_dismisses_the_menu_without_losing_the_pane(cx: &mut TestAppContext) {
    let (root, mut visual) = harness(cx, false);
    redraw(&mut visual);
    open_menu(&mut visual, shift());
    assert!(is_open(&root, &mut visual));
    visual.simulate_click(point(px(6.0), px(6.0)), Modifiers::default());
    redraw(&mut visual);
    assert!(!is_open(&root, &mut visual), "the outside click closes it");
    visual.update(|window, cx| {
        assert!(
            root.read(cx).view_focused(window, cx),
            "the click leaves the pane able to take input"
        );
    });
}

#[gpui_kit::test]
fn keyboard_copy_activation_copies_the_selected_text(cx: &mut TestAppContext) {
    let (root, mut visual) = harness(cx, false);
    redraw(&mut visual);
    // Drag across the first word so Copy is enabled.
    visual.simulate_mouse_down(
        point(px(10.0), px(8.0)),
        MouseButton::Left,
        Modifiers::default(),
    );
    visual.simulate_mouse_move(
        point(px(62.0), px(8.0)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    visual.simulate_mouse_up(
        point(px(62.0), px(8.0)),
        MouseButton::Left,
        Modifiers::default(),
    );
    redraw(&mut visual);
    let selected = visual.read(|cx| root.read(cx).model.selection_text());
    assert!(selected.as_ref().is_some_and(|text| !text.is_empty()));

    open_menu(&mut visual, Modifiers::default());
    assert!(is_open(&root, &mut visual));
    visual.simulate_keystrokes("enter");
    redraw(&mut visual);
    assert!(!is_open(&root, &mut visual), "activation closes the menu");
    assert_eq!(clip(&mut visual), selected);
    assert!(
        visual.read(|cx| root.read(cx).bytes.borrow().is_empty()),
        "Copy never writes to the session"
    );
}

#[gpui_kit::test]
fn keyboard_paste_activation_still_waits_for_the_gate(cx: &mut TestAppContext) {
    let (root, mut visual) = harness(cx, false);
    redraw(&mut visual);
    visual.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("lane\r\nnext".into())));
    open_menu(&mut visual, shift());
    assert!(is_open(&root, &mut visual));
    // With no selection Copy is disabled, so Paste is the highlighted row.
    visual.simulate_keystrokes("enter");
    redraw(&mut visual);
    let held = visual.read(|cx| root.read(cx).gate.borrow_mut().take());
    let request = held.expect("the gate received the request");
    assert_eq!(request.text, "lane\r\nnext");
    assert!(
        visual.read(|cx| root.read(cx).bytes.borrow().is_empty()),
        "no byte may bypass the gate"
    );

    visual.update(|_, cx| (request.delivery)(cx, "approved\r"));
    assert_eq!(
        visual.read(|cx| root.read(cx).bytes.borrow().clone()),
        b"approved\r",
        "only the gate's delivery reaches the session"
    );
}

#[gpui_kit::test]
fn the_menu_targets_the_clicked_pane(cx: &mut TestAppContext) {
    init(cx);
    let (window, pair) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let locale = cx.new(|_| Locale::new(Language::En));
            LocaleGlobal::set(cx, locale);
            let left = pair_pane(cx, "left");
            let right = pair_pane(cx, "right");
            window.focus(&left.view.read(cx).focus_handle(), cx);
            cx.new(|_| PairHarness { left, right })
        })
        .expect("open the two-pane window")
    });
    let mut visual = VisualTestContext::from_window(window, cx);
    redraw(&mut visual);
    let position = visual
        .debug_bounds("context-menu-pane-right")
        .expect("the right pane is laid out")
        .center();
    visual.simulate_mouse_down(position, MouseButton::Right, shift());
    redraw(&mut visual);

    visual.update(|_, cx| {
        let pair = pair.read(cx);
        assert!(!pair.left.menu.read(cx).is_open(cx));
        assert!(
            pair.right.menu.read(cx).is_open(cx),
            "the clicked pane owns it"
        );
    });
    visual.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("right".into())));
    visual.simulate_keystrokes("enter");
    redraw(&mut visual);
    visual.update(|_, cx| {
        let pair = pair.read(cx);
        assert!(
            pair.left.gate.borrow().is_none(),
            "the left pane never pasted"
        );
        assert_eq!(
            pair.right
                .gate
                .borrow()
                .as_ref()
                .map(|request| request.text.as_str()),
            Some("right")
        );
    });
}

#[gpui_kit::test]
fn a_stale_pane_target_does_not_open_a_menu(cx: &mut TestAppContext) {
    let (root, mut visual) = harness(cx, false);
    redraw(&mut visual);
    let menu = visual.read(|cx| root.read(cx).menu.clone());
    let stale = visual.update(|_, cx| {
        let model = Arc::new(Terminal::new(
            Config::default(),
            GridSize::new(80, 24),
            CellSize::new(8, 16),
        ));
        let view = cx.new(|cx| TerminalView::new(cx, model, TerminalViewConfig::default()));
        view.downgrade()
    });
    visual.update(|window, cx| {
        menu.update(cx, |menu, cx| {
            menu.open(
                ContextMenuRequest {
                    position: point(px(20.0), px(20.0)),
                    has_selection: false,
                },
                stale,
                window,
                cx,
            )
        });
    });
    assert!(
        !visual.read(|cx| menu.read(cx).is_open(cx)),
        "a closed pane has no menu to act on"
    );
}

#[gpui_kit::test]
fn labels_follow_the_shared_language_and_copy_disables_without_a_selection(
    cx: &mut TestAppContext,
) {
    init(cx);
    cx.update(|cx| {
        let locale = cx.new(|_| Locale::new(Language::En));
        LocaleGlobal::set(cx, locale);
    });
    let english = cx.update(|cx| {
        let actions = actions(true, cx);
        (
            actions[0].label.to_string(),
            actions[1].label.to_string(),
            actions[0].disabled,
            actions[1].disabled,
        )
    });
    assert_eq!(english, ("Copy".into(), "Paste".into(), false, false));

    cx.update(|cx| {
        let locale = cx.new(|_| Locale::new(Language::Zh));
        LocaleGlobal::set(cx, locale);
    });
    let chinese = cx.update(|cx| {
        let actions = actions(false, cx);
        (
            actions[0].label.to_string(),
            actions[1].label.to_string(),
            actions[0].disabled,
            actions[1].disabled,
        )
    });
    assert_eq!(chinese, ("复制".into(), "粘贴".into(), true, false));
}

/// One side of the two-pane target test.
struct PairPane {
    view: Entity<TerminalView>,
    menu: Entity<PaneContextMenu>,
    gate: GateSlot,
}

fn pair_pane(cx: &mut gpui_kit::App, side: &str) -> PairPane {
    let bytes: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let gate: GateSlot = Rc::new(RefCell::new(None));
    let menu = cx.new(|_| PaneContextMenu::new(DARK));
    let handler = super::view_handler(menu.clone());
    let gate_for_view = Rc::clone(&gate);
    let model = Arc::new(Terminal::new(
        Config::default(),
        GridSize::new(80, 24),
        CellSize::new(8, 16),
    ));
    model.write(format!("{side}\r\n").as_bytes());
    let bytes_for_view = Rc::clone(&bytes);
    let view = cx.new(|cx| {
        TerminalView::new(
            cx,
            model,
            TerminalViewConfig {
                input: Some(Rc::new(move |input| {
                    bytes_for_view.borrow_mut().extend_from_slice(input)
                })),
                context_menu: Some(handler),
                paste_gate: Some(Rc::new(move |request: PasteRequest, _, _| {
                    *gate_for_view.borrow_mut() = Some(request);
                    true
                })),
                ..TerminalViewConfig::default()
            },
        )
    });
    PairPane { view, menu, gate }
}

struct PairHarness {
    left: PairPane,
    right: PairPane,
}

impl Render for PairHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .relative()
            .flex()
            .child(pair_cell("context-menu-pane-left", &self.left))
            .child(pair_cell("context-menu-pane-right", &self.right))
    }
}

fn pair_cell(selector: &'static str, pane: &PairPane) -> impl IntoElement {
    div()
        .debug_selector(move || selector.into())
        .relative()
        .flex_basis(px(0.0))
        .flex_grow(1.0)
        .h_full()
        .child(pane.view.clone())
        .child(pane.menu.clone())
}
