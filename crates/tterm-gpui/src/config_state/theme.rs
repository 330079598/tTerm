//! Saving the built-in theme choice.

use std::sync::Arc;

use gpui_kit::{Context, Task};
use tterm_core::config::theme_file::ThemeSettings;
use tterm_core::core::blocking::run_blocking;

use super::{ConfigState, ThemeChanged, pending_save_error};

impl ConfigState {
    /// Saves the built-in theme fields as one configuration edit.
    ///
    /// The writer applies the fields that differ from this state's snapshot to
    /// the latest config file and returns the values the file now holds; the
    /// snapshot and [`ThemeChanged`] are published only after that succeeds, so
    /// the window can repaint from the same values the next launch will load.
    /// A second edit is refused while any setting save is pending. Dropping the
    /// returned task still lets the write finish and releases the pending state.
    pub fn save_theme_settings(
        &mut self,
        settings: ThemeSettings,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        if self.pending {
            return Task::ready(Err(pending_save_error()));
        }
        if settings == ThemeSettings::from(&self.config) {
            return Task::ready(Ok(()));
        }
        self.pending = true;

        let previous = self.config.clone();
        let saver = Arc::clone(&self.theme_saver);
        let runtime = self.runtime.clone();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let _save = runtime.spawn(async move {
            let result = run_blocking(move || saver(previous, settings)).await;
            let _ = result_tx.send(result);
        });

        let (waiter_tx, waiter_rx) = tokio::sync::oneshot::channel();
        cx.spawn(async move |this, cx| {
            let result = result_rx
                .await
                .unwrap_or_else(|_| Err(theme_saver_stopped()));
            let outcome = match result {
                Ok(saved) => {
                    let changed = ThemeChanged {
                        settings: saved.clone(),
                    };
                    let _ = this.update(cx, |state, cx| {
                        state.pending = false;
                        state.apply_theme_settings(&saved);
                        cx.emit(changed);
                        cx.notify();
                    });
                    Ok(())
                }
                Err(error) => {
                    let _ = this.update(cx, |state, _| state.pending = false);
                    Err(error)
                }
            };
            let _ = waiter_tx.send(outcome);
        })
        .detach();

        cx.spawn(async move |_, _| {
            waiter_rx
                .await
                .unwrap_or_else(|_| Err(theme_saver_stopped()))
        })
    }

    fn apply_theme_settings(&mut self, saved: &ThemeSettings) {
        self.config.theme = saved.theme.clone();
        self.config.theme_follow_system = saved.follow_system;
        self.config.theme_light = saved.theme_light.clone();
        self.config.theme_dark = saved.theme_dark.clone();
    }
}

fn theme_saver_stopped() -> String {
    "The theme saver stopped before it reported a result.".to_string()
}

/// A writer for tests and captures that do not persist settings: it applies
/// each theme edit to the config the state started with, so the shared rules
/// decide an edit without touching a config file.
#[cfg(any(test, all(debug_assertions, feature = "automation")))]
pub(super) fn isolated_theme_writer(
    config: &tterm_core::config::AppConfig,
) -> super::ThemeSettingsWriter {
    let config = std::sync::Mutex::new(config.clone());
    Arc::new(move |previous, settings| {
        let mut config = config
            .lock()
            .map_err(|_| "The isolated theme writer was poisoned".to_string())?;
        let previous = ThemeSettings::from(&previous);
        tterm_core::config::theme_file::apply_theme_settings(&mut config, &previous, &settings)
    })
}
