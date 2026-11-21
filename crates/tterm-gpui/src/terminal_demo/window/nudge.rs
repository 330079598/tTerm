//! Brief feedback when a pane-navigation shortcut has no destination.

use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, Window, div, px};
use tterm_ui::tokens::gpui::hsla;

use super::Prototype;

const FLASH: Duration = Duration::from_millis(340);

impl Prototype {
    pub(super) fn nudge_active_tab(&mut self, cx: &mut Context<Self>) {
        self.nudge = Some((self.tabs.active_id(), Instant::now()));
        cx.notify();
    }

    pub(super) fn tab_flash(&mut self, window: &Window, now: Instant) -> Option<Div> {
        let (id, start) = self.nudge?;
        let elapsed = now.saturating_duration_since(start);
        if elapsed >= FLASH || self.tabs.active_id() != id {
            self.nudge = None;
            return None;
        }
        let opacity = (1. - elapsed.as_secs_f32() / FLASH.as_secs_f32()).powi(3);
        let color = self.ui.theme.colors.tab_active_border;
        window.request_animation_frame();
        Some(
            div()
                .absolute()
                .inset_0()
                .rounded(px(5.))
                .bg(hsla(color, 0.24 * opacity))
                .border_1()
                .border_color(hsla(color, 0.7 * opacity)),
        )
    }
}
