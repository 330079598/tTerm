//! Dialog parity screens using the production tterm-ui Dialog primitive.
use crate::color::{hsl_to_rgba, to_gpui};
use crate::layout::{ElementSizing, LayoutSink, PaddingSnapshot, StyleSnapshot};
use crate::manifest::ScreensManifest;
use crate::marker::parity_child;
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext, Context, Entity, IntoElement, ParentElement, Styled, Window, div};
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
    input: Option<Entity<InputState>>,
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
            input: None,
        })
    }

    fn style(&self, background: [u8; 4], padding: PaddingSnapshot, radius: f32) -> StyleSnapshot {
        StyleSnapshot {
            background_color: background,
            border_color: hsl_to_rgba(self.theme.colors.border),
            border_width: 1.0,
            border_radius: radius,
            ring_spread: 0.0,
            font_family: "system-ui".into(),
            font_size: 14.0,
            font_weight: 400,
            line_height: 16.8,
            color: hsl_to_rgba(self.theme.colors.foreground),
            padding,
            declared_background_color: None,
            declared_border_color: None,
            opacity: 1.0,
        }
    }

    fn ensure_dialog(&mut self, window: &mut Window, cx: &mut Context<super::ScreenView>) {
        if self.dialog.is_some() {
            return;
        }
        let options = if self.kind == DialogKind::Prompt {
            DialogOptions::prompt("Rename profile", "Profile name")
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
        let input = if self.kind == DialogKind::Prompt {
            Some(cx.new(|cx| InputState::new(window, cx).default_value("Production")))
        } else {
            None
        };
        let dialog = cx.new(|cx| {
            let mut dialog = Dialog::new(options, self.theme, window, cx);
            if let Some(input) = &input {
                dialog = dialog.with_input(input.clone());
            }
            dialog
        });
        self.input = input;
        self.dialog = Some(dialog);
    }

    pub fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<super::ScreenView>,
    ) -> impl IntoElement {
        self.ensure_dialog(window, cx);
        let dialog = self.dialog.as_ref().expect("dialog initialized").clone();
        let id = match (self.kind, self.destructive) {
            (DialogKind::Prompt, _) => "ui.dialog.prompt.content",
            (_, true) => "ui.dialog.destructive.content",
            _ => "ui.dialog.confirm.content",
        };
        let style = self.style(
            hsl_to_rgba(self.theme.colors.background),
            PaddingSnapshot::all(24.0),
            8.0,
        );
        div()
            .size_full()
            .bg(to_gpui(hsl_to_rgba(self.theme.colors.background)))
            .child(parity_child(
                &self.layout,
                id,
                ElementSizing::Fixed,
                style,
                dialog,
            ))
    }
}
