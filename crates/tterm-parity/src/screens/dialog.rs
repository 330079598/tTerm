//! Dialog parity screens use the same production compositions as app-dialog.
use crate::color::{hsl_to_rgba, to_gpui};
use crate::layout::{BoundsSnapshot, ElementSizing, LayoutSink, PaddingSnapshot, StyleSnapshot};
use crate::manifest::ScreensManifest;
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext, Context, Entity, IntoElement, ParentElement, Styled, Window, div};
use std::rc::Rc;
use tterm_ui::components::dialog::{Dialog, DialogKind, DialogOptions};
use tterm_ui::tokens::palette::{DARK, LIGHT};
use tterm_ui::tokens::theme::Theme;
pub const CONFIRM_SCREEN_ID: &str = "ui.dialog.confirm";
pub const DESTRUCTIVE_SCREEN_ID: &str = "ui.dialog.destructive";
pub const PROMPT_SCREEN_ID: &str = "ui.dialog.prompt";
pub struct DialogScreen {
    layout: LayoutSink,
    theme: Theme,
    kind: DialogKind,
    destructive: bool,
    dialog: Option<Entity<Dialog>>,
}
impl DialogScreen {
    pub fn load(
        manifest: &ScreensManifest,
        layout: LayoutSink,
        kind: DialogKind,
        destructive: bool,
    ) -> Result<Self, String> {
        let theme = match manifest.theme.as_str() {
            "dark" => DARK,
            "light" => LIGHT,
            other => return Err(format!("unsupported theme {other}")),
        };
        Ok(Self {
            layout,
            theme,
            kind,
            destructive,
            dialog: None,
        })
    }
    fn ensure_dialog(&mut self, window: &mut Window, cx: &mut Context<super::ScreenView>) {
        if self.dialog.is_some() {
            return;
        }
        let mut options = if self.kind == DialogKind::Prompt {
            let mut options = DialogOptions::prompt("Rename profile", "");
            options.description = Some("Choose a new name for this profile.".into());
            options
        } else {
            DialogOptions::confirm(
                if self.destructive {
                    "Remove connection?"
                } else {
                    "Delete profile?"
                },
                if self.destructive {
                    "All saved credentials will be removed."
                } else {
                    "This action cannot be undone."
                },
            )
        };
        options.destructive = self.destructive;
        options.cancel_text = "Cancel".into();
        options.confirm_text = if self.destructive {
            "Delete"
        } else if self.kind == DialogKind::Prompt {
            "OK"
        } else {
            "Confirm"
        }
        .into();
        let input = if self.kind == DialogKind::Prompt {
            Some(cx.new(|cx| InputState::new(window, cx).default_value("Production")))
        } else {
            None
        };
        let prefix = if self.kind == DialogKind::Prompt {
            PROMPT_SCREEN_ID
        } else if self.destructive {
            DESTRUCTIVE_SCREEN_ID
        } else {
            CONFIRM_SCREEN_ID
        };
        let sink = self.layout.clone();
        let observer = Rc::new(
            move |slot: &str,
                  bounds: gpui_kit::Bounds<gpui_kit::Pixels>,
                  style: &tterm_ui::components::dialog::presentation::PartStyle| {
                let id = match slot {
                    "dialog-content" => format!("{prefix}.content"),
                    "dialog-overlay" => format!("{prefix}.overlay"),
                    "cancel" | "confirm" => format!("{prefix}.{slot}"),
                    _ => format!("{prefix}.content.{slot}"),
                };
                let mut background = hsl_to_rgba(style.background);
                background[3] = (style.background_alpha * 255.0).round() as u8;
                let snapshot = StyleSnapshot {
                    background_color: background,
                    border_color: hsl_to_rgba(style.border),
                    border_width: style.border_width,
                    border_radius: style.radius,
                    ring_spread: 0.0,
                    font_family: "system-ui".into(),
                    font_size: style.font_size,
                    font_weight: style.weight,
                    line_height: style.line_height,
                    color: hsl_to_rgba(style.color),
                    padding: PaddingSnapshot {
                        top: style.padding[0],
                        right: style.padding[1],
                        bottom: style.padding[2],
                        left: style.padding[3],
                    },
                    declared_background_color: None,
                    declared_border_color: None,
                    opacity: style.opacity,
                };
                sink.record_element(
                    &id,
                    ElementSizing::Fixed,
                    BoundsSnapshot::new(
                        bounds.origin.x.into(),
                        bounds.origin.y.into(),
                        bounds.size.width.into(),
                        bounds.size.height.into(),
                    ),
                    snapshot,
                );
            },
        );
        self.dialog = Some(cx.new(|cx| {
            let mut dialog = Dialog::new(options, self.theme, window, cx).observe_parts(observer);
            if let Some(input) = input {
                dialog = dialog.with_input(input);
            }
            dialog
        }));
    }
    pub fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<super::ScreenView>,
    ) -> impl IntoElement {
        self.ensure_dialog(window, cx);
        div()
            .size_full()
            .bg(to_gpui(hsl_to_rgba(self.theme.colors.background)))
            .children(self.dialog.clone())
    }
}
