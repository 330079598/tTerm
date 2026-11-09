//! Menu bindings outrank Root traversal and ancestor dialog actions.
use super::ContextMenu;
use gpui_kit::{App, Context, Div, InteractiveElement, KeyBinding, Stateful, Window};

gpui_kit::actions!(
    context_menu,
    [SelectNext, SelectPrev, First, Last, Confirm, Cancel]
);
pub(super) const CONTEXT: &str = "TTermContextMenu";

pub(super) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", SelectNext, Some(CONTEXT)),
        KeyBinding::new("shift-tab", SelectPrev, Some(CONTEXT)),
        KeyBinding::new("down", SelectNext, Some(CONTEXT)),
        KeyBinding::new("up", SelectPrev, Some(CONTEXT)),
        KeyBinding::new("home", First, Some(CONTEXT)),
        KeyBinding::new("end", Last, Some(CONTEXT)),
        KeyBinding::new("enter", Confirm, Some(CONTEXT)),
        KeyBinding::new("space", Confirm, Some(CONTEXT)),
        KeyBinding::new("escape", Cancel, Some(CONTEXT)),
    ]);
}

impl ContextMenu {
    pub(super) fn bindings(&self, card: Stateful<Div>, cx: &Context<Self>) -> Stateful<Div> {
        card.key_context(CONTEXT)
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.move_highlight(1, cx)))
            .on_action(cx.listener(|this, _: &SelectPrev, _, cx| this.move_highlight(-1, cx)))
            .on_action(cx.listener(|this, _: &First, _, cx| this.select_edge(false, cx)))
            .on_action(cx.listener(|this, _: &Last, _, cx| this.select_edge(true, cx)))
            .on_action(cx.listener(|this, _: &Confirm, window, cx| this.confirm(window, cx)))
            .on_action(cx.listener(|this, _: &Cancel, window, cx| this.close(window, cx)))
    }
    fn select_edge(&mut self, last: bool, cx: &mut Context<Self>) {
        let mut indices = self
            .actions
            .iter()
            .enumerate()
            .filter_map(|(i, a)| (!a.disabled && !a.separator).then_some(i));
        self.highlighted = if last {
            indices.next_back()
        } else {
            indices.next()
        };
        self.focus_visible = true;
        cx.notify();
    }
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.highlighted {
            self.activate(index, window, cx);
        }
    }
}
