//! The first tabs of a launch: the restored workspace, then command-line folders.
//!
//! A folder named on the command line is opened in the configured default
//! local shell with that folder as its working directory. Folders are appended
//! after the restored tabs in the order they were named, so a launch never
//! replaces the saved workspace. A folder that does not exist is left out and
//! stays visible in the window's notice bar.

use std::collections::HashMap;

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, Entity, Stateful, div, px};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationRequest};
use tterm_core::session_restore::scrollback::SavedScrollback;
use tterm_core::session_restore::workspace::WorkspaceLoad;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::tokens::gpui::hsla;

use super::super::command::CommandSpec;
use super::session_workspace;
use super::session_workspace::launch::folder_launch;
use super::session_workspace::restore::restore_workspace;
use super::{Locale, Prototype, TabFactory, Tabs, WindowTab, split_state};

/// The tab collection a window starts with, and the folders it could not open.
pub(super) struct StartupTabs {
    pub(super) tabs: Tabs<WindowTab>,
    pub(super) splits: split_state::SplitState,
    pub(super) next_saved_tab: u64,
    pub(super) missing_folders: Vec<String>,
}

/// Restores the saved workspace, then opens one tab per openable folder.
///
/// When the command names folders and no workspace is restored, the folders
/// are the whole window; the ordinary shell tab is opened only when no folder
/// can be opened. A folder tab takes a fresh persisted id that no restored tab
/// owns, and the last folder becomes active, as a later tab normally does.
pub(super) fn build_startup_tabs(
    factory: &TabFactory,
    command: CommandSpec,
    restore: bool,
    load: WorkspaceLoad,
    legacy: HashMap<String, SavedScrollback>,
    locale: &Entity<Locale>,
    cx: &mut Context<Prototype>,
) -> StartupTabs {
    let (folders, fallback) = match command {
        CommandSpec::Folders(dirs) => (dirs, CommandSpec::Shell),
        other => (Vec::new(), other),
    };
    let mut openable = Vec::new();
    let mut missing_folders = Vec::new();
    for dir in folders {
        if dir.exists() {
            openable.push(dir.path().to_string());
        } else {
            missing_folders.push(dir.path().to_string());
        }
    }

    let restored = restore
        .then(|| session_workspace::snapshot(load))
        .flatten()
        .and_then(|snapshot| restore_workspace(factory, snapshot, legacy, locale, cx));
    let config = factory.config.read(cx).config().clone();
    let mut openable = openable.into_iter();
    let mut next_saved_tab = 1;
    let (mut tabs, mut splits) = match restored {
        Some(restored) => {
            next_saved_tab = restored.next_saved_tab;
            (restored.tabs, restored.splits)
        }
        None => match openable.next() {
            Some(directory) => {
                let saved_id = Prototype::take_saved_id(&mut next_saved_tab, |_| false);
                let launch = folder_launch(saved_id, &directory, &config);
                let tab = factory.open_launch(1, launch, locale, cx);
                (Tabs::new(tab), split_state::SplitState::new(1))
            }
            None => {
                let tab = factory.open(1, fallback, locale, cx);
                next_saved_tab = 2;
                (Tabs::new(tab), split_state::SplitState::new(1))
            }
        },
    };

    for directory in openable {
        let saved_id = Prototype::take_saved_id(&mut next_saved_tab, |id| {
            tabs.ordered().any(|(_, tab)| tab.saved_id() == id)
        });
        let launch = folder_launch(saved_id, &directory, &config);
        tabs.push(|id| factory.open_launch(id, launch, locale, cx));
        splits.open(tabs.active_id());
    }

    StartupTabs {
        tabs,
        splits,
        next_saved_tab,
        missing_folders,
    }
}

/// The raw catalogue key shown when a translation lookup fails.
fn untranslated(key: tterm_core::i18n::translate::TranslationKey) -> String {
    key.as_str().to_string()
}

impl Prototype {
    /// The bar that reports folders the command line named but that do not
    /// exist. It lists every path, so the user can see what was not opened.
    pub(super) fn launch_dir_notice(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        if self.missing_folders.is_empty() {
            return None;
        }
        let (title, close_label, messages) = {
            let locale = self.ui.locale.read(cx);
            let title = locale
                .text(keys::LAUNCH_DIR_NOT_FOUND_TITLE)
                .unwrap_or_else(|error| {
                    eprintln!("tterm-gpui: failed to translate a folder notice: {error}");
                    untranslated(keys::LAUNCH_DIR_NOT_FOUND_TITLE)
                });
            let close_label = locale.text(keys::COMMON_CLOSE).unwrap_or_else(|error| {
                eprintln!("tterm-gpui: failed to translate a folder notice: {error}");
                untranslated(keys::COMMON_CLOSE)
            });
            let messages: Vec<String> = self
                .missing_folders
                .iter()
                .map(|path| {
                    let arguments = [("path", ArgumentValue::Text(path))];
                    locale
                        .translate(
                            TranslationRequest::new(keys::LAUNCH_DIR_NOT_FOUND_DESC)
                                .arguments(&arguments),
                        )
                        .unwrap_or_else(|error| {
                            eprintln!("tterm-gpui: failed to translate a folder notice: {error}");
                            format!("{}: {path}", untranslated(keys::LAUNCH_DIR_NOT_FOUND_DESC))
                        })
                })
                .collect();
            (title, close_label, messages)
        };

        let theme = self.ui.theme;
        let colors = theme.colors;
        let close = button(
            "launch-dir-notice-close",
            close_label,
            Variant::Ghost,
            ButtonSize::Xs,
            ButtonState::default(),
            theme,
        )
        .debug_selector(|| "launch-dir-notice-close".to_string())
        .on_click(cx.listener(|this, _, _, cx| {
            this.missing_folders.clear();
            cx.notify();
        }));
        let mut bar = div()
            .id("launch-dir-notice")
            .debug_selector(|| "launch-dir-notice".to_string())
            .flex()
            .flex_col()
            .gap(px(4.0))
            .px(px(12.0))
            .py(px(8.0))
            .border_b(px(1.0))
            .border_color(hsla(colors.destructive, 0.45))
            .bg(hsla(colors.destructive, 0.12))
            .text_color(hsla(colors.foreground, 1.0))
            .text_sm()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(div().flex_1().min_w_0().child(title))
                    .child(close),
            );
        for message in messages {
            bar = bar.child(div().child(message));
        }
        Some(bar)
    }
}
