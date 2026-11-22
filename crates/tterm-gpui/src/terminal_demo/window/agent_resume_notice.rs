//! The active tab's agent-resume notice and its manual continue action.

use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Div, div, px};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::tokens::gpui::hsla;

use super::Prototype;
use super::agent_resume::state::{Notice, NoticeReason};

impl Prototype {
    fn translated(&self, key: TranslationKey, cx: &App) -> String {
        self.ui.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate a workspace label: {error}");
            key.as_str().to_string()
        })
    }

    pub(super) fn agent_resume_notice(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::Stateful<Div>> {
        let notice: Notice = self
            .tabs
            .active()
            .agent_resume_controller
            .as_ref()?
            .notice()?;
        let key = match notice.reason {
            NoticeReason::Timeout => keys::AGENT_RESUME_TIMEOUT,
            NoticeReason::UnknownShell => keys::AGENT_RESUME_UNKNOWN_SHELL,
            NoticeReason::Unavailable => keys::AGENT_RESUME_UNAVAILABLE,
            NoticeReason::Unsafe => keys::AGENT_RESUME_UNSAFE,
            NoticeReason::Failed => keys::AGENT_RESUME_FAILED,
        };
        let message = self.translated(key, cx);
        let theme = self.ui.theme;
        let colors = theme.colors;
        let tab_id = self.tabs.active_id();
        let mut bar = div()
            .id("agent-resume-notice")
            .flex()
            .items_center()
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(8.0))
            .border_b(px(1.0))
            .border_color(hsla(colors.warning, 0.45))
            .bg(hsla(colors.warning, 0.14))
            .text_color(hsla(colors.foreground, 1.0))
            .text_sm()
            .child(div().flex_1().min_w_0().child(message));
        if notice.enabled {
            let label = self.translated(keys::AGENT_RESUME_CONTINUE, cx);
            bar = bar.child(
                button(
                    "agent-resume-continue",
                    label,
                    Variant::Default,
                    ButtonSize::Xs,
                    ButtonState::default(),
                    theme,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(controller) = this
                        .tabs
                        .get_mut(tab_id)
                        .and_then(|tab| tab.agent_resume_controller.as_mut())
                    {
                        controller.resume_manually(cx);
                    }
                })),
            );
        }
        Some(bar)
    }
}
