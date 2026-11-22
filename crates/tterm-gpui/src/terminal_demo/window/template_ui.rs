//! The native split-layout picker: built-ins, saved layouts, and saving the
//! shape of the tab the picker was opened from.

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Div, Entity, FocusHandle, MouseButton, Role, Stateful, Subscription, Window, div,
    px,
};
use tterm_core::config::panes::PaneLayoutTemplate as ConfigTemplate;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationKey, TranslationRequest};
use tterm_core::keymap::actions::KeymapActionId;
use tterm_core::workspace::pane_templates::{
    BUILTIN_PANE_TEMPLATES, LayoutShape, PaneLayoutTemplate, count_template_panes,
    normalize_pane_templates, shape_of_tree,
};
use tterm_core::workspace::tab_groups::{PaneNode, get_pane_ids};
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::input::{InputOptions, input};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use super::{KeymapRouter, Prototype};
use crate::config_state::ConfigState;
use crate::locale::Locale;

mod cards;
mod preview;
mod render;
#[cfg(test)]
mod tests;
mod wiring;

const DIALOG_ID: &str = "layout-template-dialog";
const OPEN_BUTTON_ID: &str = "layout-template-open";
const CANCEL_BUTTON_ID: &str = "layout-template-cancel";
const SAVE_BUTTON_ID: &str = "layout-template-save";
const NAME_FIELD_ID: &str = "layout-template-name";
const FEEDBACK_ID: &str = "layout-template-feedback";
const DIALOG_WIDTH: f32 = 620.0;
const DIALOG_PADDING: f32 = 24.0;
const DIALOG_RADIUS: f32 = 8.0;
const SAVED_LIST_MAX_HEIGHT: f32 = 220.0;

type ApplyTemplate = Rc<dyn Fn(usize, &LayoutShape, &mut Window, &mut App) -> bool>;

#[derive(Clone, PartialEq)]
struct TemplateTarget {
    tab: usize,
    root: PaneNode,
    can_copy: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SavePurpose {
    Add,
    Delete,
}

#[derive(Clone, Copy)]
enum Shortcut {
    Close,
    Save,
}

/// The picker entity owned by the workspace window.
pub(super) struct TemplateUi {
    config: Entity<ConfigState>,
    locale: Entity<Locale>,
    theme: Theme,
    apply: ApplyTemplate,
    focus_handle: FocusHandle,
    target: Option<TemplateTarget>,
    open: bool,
    input: Option<Entity<InputState>>,
    feedback: Option<String>,
    saving: bool,
    save_purpose: Option<SavePurpose>,
    restore_focus: Option<FocusHandle>,
    key_interceptor: Option<Subscription>,
}

impl TemplateUi {
    /// Creates the picker for the workspace, routing applications through the
    /// prototype that owns the live tab cards.
    pub(super) fn for_workspace(
        config: Entity<ConfigState>,
        locale: Entity<Locale>,
        theme: Theme,
        cx: &mut Context<Prototype>,
    ) -> Entity<Self> {
        let workspace = cx.weak_entity();
        let apply: ApplyTemplate = Rc::new(move |tab, shape, window, cx| {
            let shape = shape.clone();
            workspace
                .update(cx, |prototype, cx| {
                    if tab == prototype.tabs.active_id() {
                        prototype.apply_layout_template(&shape, window, cx)
                    } else {
                        prototype.apply_layout_template_to(tab, &shape, window, cx)
                    }
                })
                .unwrap_or(false)
        });
        cx.new(|cx| Self::new(config, locale, theme, apply, cx))
    }

    fn new(
        config: Entity<ConfigState>,
        locale: Entity<Locale>,
        theme: Theme,
        apply: ApplyTemplate,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&config, |_, _, cx| cx.notify()).detach();
        cx.observe(&locale, |_, _, cx| cx.notify()).detach();
        Self {
            config,
            locale,
            theme,
            apply,
            focus_handle: cx.focus_handle(),
            target: None,
            open: false,
            input: None,
            feedback: None,
            saving: false,
            save_purpose: None,
            restore_focus: None,
            key_interceptor: None,
        }
    }

    fn open(&mut self, target: TemplateTarget, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        if self.open {
            self.target = Some(target);
            self.feedback = None;
            self.focus_input(window, cx);
            cx.notify();
            return;
        }
        self.restore_focus = window.focused(cx);
        self.intercept_shortcuts(window, cx);
        let pane_count = get_pane_ids(&target.root).len();
        let placeholder = self.untitled_name(pane_count, cx);
        self.target = Some(target);
        self.feedback = None;
        self.save_purpose = None;
        let state = cx.new(|cx| InputState::new(window, cx));
        state.update(cx, |state, cx| {
            state.set_placeholder(placeholder, window, cx);
            state.set_value("", window, cx);
            state.focus(window, cx);
        });
        self.input = Some(state);
        self.open = true;
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.open = false;
        self.target = None;
        self.feedback = None;
        self.input = None;
        self.stop_intercepting_shortcuts();
        if let Some(focus) = self.restore_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    /// Hides the picker when the workspace switches to another route.
    ///
    /// Unlike [`Self::close`], this dismisses the overlay even while a save is
    /// in flight. The save keeps running, but its key interceptor and the
    /// hidden name field must not keep control of the keyboard.
    pub(super) fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        self.target = None;
        self.feedback = None;
        self.input = None;
        self.restore_focus = None;
        self.stop_intercepting_shortcuts();
        cx.notify();
    }

    fn target_tab(&self) -> Option<usize> {
        self.target.as_ref().map(|target| target.tab)
    }

    fn sync_target(&mut self, target: Option<TemplateTarget>, cx: &mut Context<Self>) {
        if self.target == target {
            return;
        }
        if target.is_none() && self.open {
            self.open = false;
            self.input = None;
            self.feedback = None;
            self.restore_focus = None;
            self.stop_intercepting_shortcuts();
            cx.notify();
        }
        self.target = target;
    }

    fn apply_shape(&mut self, shape: &LayoutShape, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.target.clone() else {
            return;
        };
        let apply = Rc::clone(&self.apply);
        if apply(target.tab, shape, window, cx) {
            self.open = false;
            self.target = None;
            self.input = None;
            self.feedback = None;
            self.restore_focus = None;
            self.stop_intercepting_shortcuts();
            cx.notify();
        } else {
            self.feedback = Some(self.text(cx, keys::LAYOUT_TEMPLATES_NEEDS_SESSION));
            self.focus_input(window, cx);
            cx.notify();
        }
    }

    fn submit_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(target) = &self.target else {
            return;
        };
        let pane_count = get_pane_ids(&target.root).len();
        if pane_count < 2 {
            self.feedback = Some(self.text(cx, keys::LAYOUT_TEMPLATES_SPLIT_FIRST));
            cx.notify();
            return;
        }
        let entered = self
            .input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let name = entered.trim();
        let name = if name.is_empty() {
            self.untitled_name(pane_count, cx)
        } else {
            name.to_string()
        };
        let mut templates = self.saved_templates(cx);
        templates.push(PaneLayoutTemplate {
            id: new_template_id(),
            name,
            root: shape_of_tree(&target.root),
        });
        self.save_templates(templates, SavePurpose::Add, window, cx);
    }

    fn delete_template(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let templates = self
            .saved_templates(cx)
            .into_iter()
            .filter(|template| template.id != id)
            .collect();
        self.save_templates(templates, SavePurpose::Delete, window, cx);
    }

    fn save_templates(
        &mut self,
        templates: Vec<PaneLayoutTemplate>,
        purpose: SavePurpose,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let config_templates = match config_templates(&templates) {
            Ok(templates) => templates,
            Err(message) => {
                self.feedback = Some(message);
                cx.notify();
                return;
            }
        };
        self.saving = true;
        self.save_purpose = Some(purpose);
        self.feedback = None;
        cx.notify();
        let task = self.config.update(cx, |config, cx| {
            config.save_pane_templates(config_templates, cx)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|window, app| {
                let _ = this.update(app, |this, cx| this.finish_save(result, window, cx));
            });
        })
        .detach();
    }

    fn finish_save(
        &mut self,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.saving = false;
        let purpose = self.save_purpose.take();
        match result {
            Ok(()) => {
                self.feedback = None;
                if purpose == Some(SavePurpose::Add)
                    && self.open
                    && let Some(input) = &self.input
                {
                    input.update(cx, |input, cx| {
                        input.set_value("", window, cx);
                        input.focus(window, cx);
                    });
                }
                cx.notify();
            }
            Err(message) => {
                self.feedback = Some(message);
                if self.open {
                    self.focus_input(window, cx);
                }
                cx.notify();
            }
        }
    }

    fn focus_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.input {
            input.update(cx, |input, cx| input.focus(window, cx));
        }
    }

    fn saved_templates(&self, cx: &App) -> Vec<PaneLayoutTemplate> {
        let value = serde_json::to_value(&self.config.read(cx).config().panes.templates)
            .unwrap_or(serde_json::Value::Null);
        normalize_pane_templates(&value)
    }

    fn untitled_name(&self, pane_count: usize, cx: &App) -> String {
        self.text_count(cx, keys::LAYOUT_TEMPLATES_UNTITLED, pane_count)
    }

    fn pane_count_text(&self, pane_count: usize, cx: &App) -> String {
        self.text_count(cx, keys::LAYOUT_TEMPLATES_PANE_COUNT, pane_count)
    }

    fn builtin_label(&self, id: &str, cx: &App) -> String {
        let key = match id {
            "builtin:columns-2" => keys::LAYOUT_TEMPLATES_BUILTIN_COLUMNS_2,
            "builtin:rows-2" => keys::LAYOUT_TEMPLATES_BUILTIN_ROWS_2,
            "builtin:columns-3" => keys::LAYOUT_TEMPLATES_BUILTIN_COLUMNS_3,
            "builtin:main-right-2" => keys::LAYOUT_TEMPLATES_BUILTIN_MAIN_RIGHT_2,
            "builtin:main-bottom-2" => keys::LAYOUT_TEMPLATES_BUILTIN_MAIN_BOTTOM_2,
            "builtin:grid-4" => keys::LAYOUT_TEMPLATES_BUILTIN_GRID_4,
            _ => keys::LAYOUT_TEMPLATES_BUILTIN,
        };
        self.text(cx, key)
    }

    fn delete_label(&self, name: &str, cx: &App) -> String {
        let arguments = [("name", ArgumentValue::Text(name))];
        self.locale
            .read(cx)
            .translate(TranslationRequest::new(keys::LAYOUT_TEMPLATES_DELETE).arguments(&arguments))
            .unwrap_or_else(|error| {
                eprintln!("tterm-gpui: failed to translate a layout delete label: {error}");
                keys::LAYOUT_TEMPLATES_DELETE.as_str().to_string()
            })
    }

    fn text_count(&self, cx: &App, key: TranslationKey, count: usize) -> String {
        self.locale
            .read(cx)
            .translate(TranslationRequest::new(key).count(count as f64))
            .unwrap_or_else(|error| {
                eprintln!("tterm-gpui: failed to translate a layout template label: {error}");
                format!("{} ({count})", key.as_str())
            })
    }

    fn text(&self, cx: &App, key: TranslationKey) -> String {
        self.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate a layout template label: {error}");
            key.as_str().to_string()
        })
    }

    /// Claims Escape and Enter before the name field's key bindings see them.
    fn intercept_shortcuts(&mut self, window: &Window, cx: &mut Context<Self>) {
        let this = cx.weak_entity();
        let handle = window.window_handle();
        self.key_interceptor = Some(cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle() != handle {
                return;
            }
            let action = match event.keystroke.key.as_str() {
                "escape" => Shortcut::Close,
                "enter" => Shortcut::Save,
                _ => return,
            };
            let _ = this.update(cx, |this, cx| {
                if !this.open {
                    return;
                }
                cx.stop_propagation();
                window.prevent_default();
                match action {
                    Shortcut::Close => this.close(window, cx),
                    Shortcut::Save => this.submit_save(window, cx),
                }
            });
        }));
    }

    fn stop_intercepting_shortcuts(&mut self) {
        self.key_interceptor = None;
    }

    fn builtin_section(&self, cx: &mut Context<Self>) -> Div {
        let Some(target) = &self.target else {
            return div();
        };
        let current_panes = get_pane_ids(&target.root).len();
        let theme = self.theme;
        let mut grid = div().flex().flex_wrap().gap(px(8.0));
        for template in BUILTIN_PANE_TEMPLATES.iter() {
            let panes = count_template_panes(&template.root);
            let disabled = panes > current_panes && !target.can_copy;
            let sanitized = sanitize_id(template.id);
            let selector = format!("layout-template-builtin-{sanitized}");
            let card = cards::card(
                cards::CardSpec {
                    id: selector.clone(),
                    selector,
                    label: self.builtin_label(template.id, cx),
                    pane_count: self.pane_count_text(panes, cx),
                    shape: &template.root,
                    disabled,
                    needs_session: disabled
                        .then(|| self.text(cx, keys::LAYOUT_TEMPLATES_NEEDS_SESSION)),
                },
                theme,
            );
            let shape = template.root.clone();
            grid = grid.child(card.when(!disabled, |card| {
                card.on_click(cx.listener(move |this, _, window, cx| {
                    this.apply_shape(&shape, window, cx);
                }))
            }));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(section_heading(
                self.text(cx, keys::LAYOUT_TEMPLATES_BUILTIN),
                theme,
            ))
            .child(grid)
    }

    fn saved_section(&self, cx: &mut Context<Self>) -> Div {
        let Some(target) = &self.target else {
            return div();
        };
        let theme = self.theme;
        let saved = self.saved_templates(cx);
        let current_panes = get_pane_ids(&target.root).len();
        let mut section = div().flex().flex_col().gap(px(8.0)).child(section_heading(
            self.text(cx, keys::LAYOUT_TEMPLATES_SAVED),
            theme,
        ));
        if saved.is_empty() {
            return section.child(
                div()
                    .text_size(px(12.0))
                    .text_color(hsla(theme.colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::LAYOUT_TEMPLATES_NONE_SAVED)),
            );
        }
        let mut grid = div().flex().flex_wrap().gap(px(8.0));
        for template in saved {
            let panes = count_template_panes(&template.root);
            let disabled = panes > current_panes && !target.can_copy;
            let sanitized = sanitize_id(&template.id);
            let card = cards::card(
                cards::CardSpec {
                    id: format!("layout-template-saved-{sanitized}"),
                    selector: format!("layout-template-saved-{sanitized}"),
                    label: template.name.clone(),
                    pane_count: self.pane_count_text(panes, cx),
                    shape: &template.root,
                    disabled,
                    needs_session: disabled
                        .then(|| self.text(cx, keys::LAYOUT_TEMPLATES_NEEDS_SESSION)),
                },
                theme,
            );
            let shape = template.root.clone();
            let card = card.when(!disabled, |card| {
                card.on_click(cx.listener(move |this, _, window, cx| {
                    this.apply_shape(&shape, window, cx);
                }))
            });
            let delete_selector = format!("layout-template-delete-{sanitized}");
            let delete_label = self.delete_label(&template.name, cx);
            let delete_id = template.id.clone();
            let delete = button(
                delete_selector.clone(),
                "×",
                Variant::Ghost,
                ButtonSize::IconXs,
                ButtonState::default(),
                theme,
            )
            .debug_selector(move || delete_selector.clone())
            .aria_label(delete_label)
            .absolute()
            .top(px(4.0))
            .right(px(4.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.delete_template(&delete_id, window, cx);
            }));
            grid = grid.child(div().relative().child(card).child(delete));
        }
        section = section.child(
            div()
                .id("layout-template-saved-list")
                .max_h(px(SAVED_LIST_MAX_HEIGHT))
                .overflow_y_scroll()
                .child(grid),
        );
        section
    }

    fn save_section(&self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let section = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .pt(px(12.0))
            .border_t(px(1.0))
            .border_color(hsla(theme.colors.border, 1.0))
            .child(section_heading(
                self.text(cx, keys::LAYOUT_TEMPLATES_SAVE_CURRENT),
                theme,
            ));
        let Some(target) = &self.target else {
            return section;
        };
        if get_pane_ids(&target.root).len() < 2 {
            return section.child(
                div()
                    .text_size(px(12.0))
                    .text_color(hsla(theme.colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::LAYOUT_TEMPLATES_SPLIT_FIRST)),
            );
        }
        let name_label = self.text(cx, keys::LAYOUT_TEMPLATES_NAME);
        let mut field = div()
            .id(NAME_FIELD_ID)
            .debug_selector(|| NAME_FIELD_ID.to_string())
            .aria_label(name_label)
            .flex_1()
            .min_w_0();
        if let Some(input_state) = &self.input {
            field = field.child(input(
                input_state,
                InputOptions {
                    theme: Some(theme),
                    ..InputOptions::default()
                },
            ));
        }
        let save_label = self.text(cx, keys::LAYOUT_TEMPLATES_SAVE);
        let save = button(
            SAVE_BUTTON_ID,
            save_label.clone(),
            Variant::Default,
            ButtonSize::Default,
            ButtonState {
                disabled: self.saving,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| SAVE_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(save_label)
        .when(!self.saving, |save| {
            save.on_click(cx.listener(|this, _, window, cx| this.submit_save(window, cx)))
        });
        section.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(field)
                .child(save),
        )
    }
}

/// Registers the picker's keymap action before terminal encoding sees it.
pub(super) fn register_actions(router: &KeymapRouter) {
    router.register(KeymapActionId::WorkspaceChooseLayout, Box::new(|_, _| true));
}

fn config_templates(templates: &[PaneLayoutTemplate]) -> Result<Vec<ConfigTemplate>, String> {
    templates
        .iter()
        .map(|template| {
            Ok(ConfigTemplate {
                id: template.id.clone(),
                name: template.name.clone(),
                root: serde_json::to_value(&template.root)
                    .map_err(|error| format!("Failed to encode the split layout: {error}"))?,
            })
        })
        .collect()
}

fn section_heading(label: String, theme: Theme) -> Div {
    div()
        .text_size(px(12.0))
        .font_weight(gpui_kit::FontWeight(500.0))
        .text_color(hsla(theme.colors.muted_foreground, 1.0))
        .child(label)
}

fn feedback_line(message: String, theme: Theme) -> Stateful<Div> {
    div()
        .id(FEEDBACK_ID)
        .debug_selector(|| FEEDBACK_ID.to_string())
        .role(Role::Status)
        .aria_label(message.clone())
        .text_size(px(12.0))
        .text_color(hsla(theme.colors.destructive, 1.0))
        .child(message)
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn new_template_id() -> String {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!(
        "saved-{nanos:x}-{:x}",
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    )
}
