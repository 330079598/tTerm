//! Production context-menu entity, using the same generated page-tab fixture.
use crate::layout::{BoundsSnapshot, ElementSizing, LayoutSink, PaddingSnapshot, StyleSnapshot};
use crate::manifest::{ScreenSpec, ScreensManifest};
use gpui_kit::{AppContext, Context, Entity, IntoElement, ParentElement, Styled, Window, div};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path, rc::Rc};
use tterm_ui::components::context_menu::{ContextMenu, MenuAction, MenuIcon, PartStyle};
use tterm_ui::tokens::palette::{DARK, LIGHT};
use tterm_ui::tokens::theme::Theme;

pub const DEFAULT_ID: &str = "ui.context-menu.default";
pub const WRAPPED_ID: &str = "ui.context-menu.wrapped";
pub const HIGHLIGHTED_ID: &str = "ui.context-menu.highlighted";

#[derive(Deserialize)]
struct Fixture {
    actions: Vec<Action>,
    shortcuts: BTreeMap<String, Vec<Option<String>>>,
}
#[derive(Deserialize)]
struct Action {
    label: String,
    action: String,
    icon: Option<String>,
    #[serde(default)]
    separator: bool,
    #[serde(default)]
    disabled: bool,
}

pub struct ContextMenuScreen {
    theme: Theme,
    layout: LayoutSink,
    actions: Vec<MenuAction>,
    highlighted: bool,
    menu: Option<Entity<ContextMenu>>,
}
impl ContextMenuScreen {
    pub fn load(
        root: &Path,
        screen: &ScreenSpec,
        manifest: &ScreensManifest,
        layout: LayoutSink,
    ) -> Result<Self, String> {
        let theme = match manifest.theme.as_str() {
            "dark" => DARK,
            "light" => LIGHT,
            other => return Err(format!("unsupported theme {other}")),
        };
        let fixture: Fixture = serde_json::from_slice(
            &std::fs::read(ScreensManifest::fixture_path(root, screen))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let platform = if cfg!(target_os = "windows") {
            "windows"
        } else {
            "macos"
        };
        let shortcuts = fixture
            .shortcuts
            .get(platform)
            .ok_or("missing platform shortcuts")?;
        let actions = fixture
            .actions
            .into_iter()
            .enumerate()
            .map(|(i, a)| {
                let icon = match a.icon.as_deref() {
                    None => None,
                    Some("plus") => Some(MenuIcon::Plus),
                    Some("rotate-ccw") => Some(MenuIcon::Reopen),
                    Some("x") => Some(MenuIcon::Close),
                    Some(other) => return Err(format!("unmigrated icon {other}")),
                };
                Ok(MenuAction {
                    label: a.label.into(),
                    action: a.action.into(),
                    icon,
                    separator: a.separator,
                    disabled: a.disabled,
                    shortcut: shortcuts.get(i).cloned().flatten().map(Into::into),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            theme,
            layout,
            actions,
            highlighted: screen.id == HIGHLIGHTED_ID,
            menu: None,
        })
    }

    pub fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<super::ScreenView>,
    ) -> impl IntoElement {
        if self.menu.is_none() {
            let sink = self.layout.clone();
            let observer = Rc::new(
                move |slot: &str, bounds: gpui_kit::Bounds<gpui_kit::Pixels>, part: PartStyle| {
                    let id = if slot.is_empty() {
                        "ui.context-menu".into()
                    } else {
                        format!("ui.context-menu.{slot}")
                    };
                    if slot.starts_with("text-") || slot.starts_with("shortcut-") {
                        sink.record_text_region(
                            slot,
                            BoundsSnapshot::new(
                                bounds.origin.x.as_f32() + part.padding[3],
                                bounds.origin.y.as_f32(),
                                bounds.size.width.as_f32() - part.padding[3],
                                bounds.size.height.as_f32(),
                            ),
                        );
                        if slot.starts_with("text-") {
                            return;
                        }
                    }
                    sink.record_element(
                        &id,
                        ElementSizing::Fixed,
                        BoundsSnapshot::new(
                            bounds.origin.x.as_f32(),
                            bounds.origin.y.as_f32(),
                            bounds.size.width.as_f32(),
                            bounds.size.height.as_f32(),
                        ),
                        snapshot(part),
                    );
                },
            );
            let actions = self.actions.clone();
            let theme = self.theme;
            let highlighted = self.highlighted;
            self.menu = Some(cx.new(|cx| {
                ContextMenu::new("parity-menu", (760.0, 550.0), actions, theme, window, cx)
                    .observe(observer)
            }));
            window.defer(cx, move |window, cx| {
                // Match React's real keyboard event path, including focus-visible modality.
                for key in if highlighted {
                    vec!["home", "down"]
                } else {
                    vec!["home"]
                } {
                    if let Ok(keystroke) = gpui_kit::Keystroke::parse(key) {
                        window.dispatch_keystroke(keystroke, cx);
                    }
                }
            });
        }
        div()
            .size_full()
            .bg(tterm_ui::tokens::gpui::hsla(
                self.theme.colors.background,
                1.0,
            ))
            .children(self.menu.clone())
    }
}

fn rgba(color: gpui_kit::Hsla) -> [u8; 4] {
    let c = gpui_kit::Rgba::from(color);
    [c.r, c.g, c.b, c.a].map(|v| (v * 255.0).round() as u8)
}
fn snapshot(part: PartStyle) -> StyleSnapshot {
    let [top, right, bottom, left] = part.padding;
    StyleSnapshot {
        background_color: rgba(part.background),
        border_color: rgba(part.border),
        border_width: part.border_width,
        border_radius: part.radius,
        ring_spread: part.ring_spread,
        font_family: part.font_family.into(),
        font_size: part.font_size,
        font_weight: part.font_weight,
        line_height: part.line_height,
        color: rgba(part.foreground),
        padding: PaddingSnapshot {
            top,
            right,
            bottom,
            left,
        },
        opacity: part.opacity * part.ancestor_opacity,
        declared_background_color: None,
        declared_border_color: None,
    }
}
