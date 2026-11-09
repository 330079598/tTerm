//! Locally themed counterpart of the Card/ghost Button context menu.
//!
//! gpui-component's PopupMenu paints selection and disabled colours from its
//! global theme. This menu instead preserves the production focus ring and
//! ghost hover colours without changing any global theme.

mod keyboard;
pub mod placement;
#[cfg(test)]
mod tests;
mod text;
mod view;

use crate::tokens::theme::Theme;
use gpui_kit::{App, Bounds, Context, FocusHandle, Hsla, Pixels, SharedString, Window};
use std::{cell::Cell, rc::Rc};

/// Registers menu bindings during application setup and after rebuilding the keymap.
pub fn init(cx: &mut App) {
    keyboard::init(cx);
}

/// Only the icons currently used by the page-tab menu are migrated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuIcon {
    Plus,
    Reopen,
    Close,
}

/// A menu row or separator. Shortcut labels are supplied by the caller's keymap.
#[derive(Clone, Debug)]
pub struct MenuAction {
    pub label: SharedString,
    pub action: SharedString,
    pub icon: Option<MenuIcon>,
    pub separator: bool,
    pub disabled: bool,
    pub shortcut: Option<SharedString>,
}

/// Paint and layout declarations shared by the renderer and parity observer.
#[derive(Clone, Copy, Debug)]
pub struct PartStyle {
    pub background: Hsla,
    pub border: Hsla,
    pub foreground: Hsla,
    pub border_width: f32,
    pub radius: f32,
    pub ring_spread: f32,
    pub font_size: f32,
    pub line_height: f32,
    pub padding: [f32; 4],
    pub opacity: f32,
    pub ancestor_opacity: f32,
    pub font_family: &'static str,
    pub font_weight: u16,
}

/// Receives each actual child border box after layout, including internal slots.
pub type PartObserver = Rc<dyn Fn(&str, Bounds<Pixels>, PartStyle)>;
type ActionCallback = Rc<dyn Fn(&str, &mut Window, &mut App)>;
type CloseCallback = Rc<dyn Fn(&mut Window, &mut App)>;

/// Interactive menu entity; attach it as a child anywhere in the window.
/// Placement is always relative to the window, including transformed parents.
pub struct ContextMenu {
    id: SharedString,
    origin: (f32, f32),
    position: Rc<Cell<Option<gpui_kit::Point<Pixels>>>>,
    actions: Vec<MenuAction>,
    theme: Theme,
    highlighted: Option<usize>,
    focus_visible: bool,
    hovered: Option<usize>,
    open: bool,
    focus: FocusHandle,
    previous_focus: Option<FocusHandle>,
    on_action: ActionCallback,
    on_close: CloseCallback,
    observer: Option<PartObserver>,
}

impl ContextMenu {
    /// Creates an open menu and takes keyboard focus, as the React portal does.
    pub fn new(
        id: impl Into<SharedString>,
        origin: (f32, f32),
        actions: Vec<MenuAction>,
        theme: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let highlighted = actions.iter().position(|a| !a.separator && !a.disabled);
        let previous_focus = window.focused(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            id: id.into(),
            origin,
            position: Rc::new(Cell::new(None)),
            actions,
            theme,
            highlighted,
            focus_visible: window.last_input_was_keyboard(),
            hovered: None,
            open: true,
            focus,
            previous_focus,
            on_action: Rc::new(|_, _, _| {}),
            on_close: Rc::new(|_, _| {}),
            observer: None,
        }
    }

    /// Called once for an enabled row's pointer or keyboard activation.
    pub fn on_action(mut self, callback: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Rc::new(callback);
        self
    }
    /// Called once when the menu closes, after an action callback if selected.
    pub fn on_close(mut self, callback: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Rc::new(callback);
        self
    }
    /// Observes actual production layout; it does not replace any rendering.
    pub fn observe(mut self, observer: PartObserver) -> Self {
        self.observer = Some(observer);
        self
    }
    /// A changed pointer origin starts a new placement measurement, like React's [x, y] effect.
    pub fn set_origin(&mut self, origin: (f32, f32), cx: &mut Context<Self>) {
        if self.origin != origin {
            self.origin = origin;
            self.position.set(None);
            cx.notify();
        }
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        if (self.focus.is_focused(window) || window.focused(cx).is_none())
            && let Some(previous) = &self.previous_focus
        {
            previous.focus(window, cx);
        }
        let callback = self.on_close.clone();
        window.defer(cx, move |window, cx| callback(window, cx));
        cx.notify();
    }

    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        if let Some(action) = self
            .actions
            .get(index)
            .filter(|a| !a.separator && !a.disabled)
        {
            let callback = self.on_action.clone();
            let action = action.action.clone();
            window.defer(cx, move |window, cx| callback(&action, window, cx));
            self.close(window, cx);
        }
    }

    fn move_highlight(&mut self, step: isize, cx: &mut Context<Self>) {
        let enabled: Vec<_> = self
            .actions
            .iter()
            .enumerate()
            .filter_map(|(i, a)| (!a.disabled && !a.separator).then_some(i))
            .collect();
        if enabled.is_empty() {
            return;
        }
        let pos = self
            .highlighted
            .and_then(|index| enabled.iter().position(|&i| i == index));
        let next = match pos {
            Some(pos) => (pos as isize + step).rem_euclid(enabled.len() as isize) as usize,
            None if step < 0 => enabled.len() - 1,
            None => 0,
        };
        self.highlighted = Some(enabled[next]);
        self.focus_visible = true;
        cx.notify();
    }
}
