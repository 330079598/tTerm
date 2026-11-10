use tauri::AppHandle;
use tterm_platform::tray::{Tray, TrayModel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct RemovalState {
    removing: bool,
    deferred: Option<TrayModel>,
}
impl RemovalState {
    fn begin_remove(&mut self) -> bool {
        self.deferred = None;
        if self.removing {
            return false;
        }
        self.removing = true;
        true
    }
    fn defer_present(&mut self, model: TrayModel) -> Option<TrayModel> {
        if self.removing {
            self.deferred = Some(model);
            None
        } else {
            Some(model)
        }
    }
    fn is_present(&self, installed: bool) -> bool {
        installed && !self.removing
    }
}

pub(super) struct TauriTray {
    app: AppHandle,
    removal: Arc<Mutex<RemovalState>>,
}
impl TauriTray {
    pub(super) fn new(app: AppHandle) -> Self {
        Self { app, removal: Arc::default() }
    }
}
impl Tray for TauriTray {
    fn present(&self, model: TrayModel) -> Result<(), String> {
        let model = self.removal.lock().unwrap_or_else(|error| error.into_inner())
            .defer_present(model);
        if let Some(model) = model {
            present(&self.app, model)?;
        }
        Ok(())
    }
    fn remove(&self) -> Result<(), String> {
        if !self.removal.lock().unwrap_or_else(|error| error.into_inner()).begin_remove() {
            return Ok(());
        }
        // The final status-item reference is destroyed on its owning UI thread.
        // Presents received before that happens are replayed after destruction.
        let app = self.app.clone();
        let removal = self.removal.clone();
        if let Err(error) = self.app.run_on_main_thread(move || {
            crate::background::tray::hide(&app);
            let mut removal = removal.lock().unwrap_or_else(|error| error.into_inner());
            if let Some(model) = removal.deferred.take() {
                if let Err(error) = present(&app, model) {
                    eprintln!("Failed to restore the tray after removal: {error}");
                }
            }
            removal.removing = false;
        }) {
            let mut removal = self.removal.lock().unwrap_or_else(|error| error.into_inner());
            removal.removing = false;
            removal.deferred = None;
            return Err(error.to_string());
        }
        Ok(())
    }
    fn is_present(&self) -> bool {
        self.removal.lock().unwrap_or_else(|error| error.into_inner())
            .is_present(self.app.tray_by_id(crate::background::tray::TRAY_ID).is_some())
    }
    fn set_tooltip(&self, text: String) -> Result<(), String> {
        if let Some(tray) = self.app.tray_by_id(crate::background::tray::TRAY_ID) {
            tray.set_tooltip(Some(text)).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

fn present(app: &AppHandle, model: TrayModel) -> Result<(), String> {
    let menu = build_menu(app, &model.entries).map_err(|error| error.to_string())?;
    crate::background::tray::present_menu(app, menu).map_err(|error| error.to_string())
}

fn build_menu(
    app: &AppHandle,
    entries: &[tterm_platform::tray::MenuEntry],
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let menu = tauri::menu::Menu::new(app)?;
    for item in menu_items(app, entries)? {
        menu.append(item.as_ref())?;
    }
    Ok(menu)
}
fn menu_items(
    app: &AppHandle,
    entries: &[tterm_platform::tray::MenuEntry],
) -> tauri::Result<Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>>> {
    use tauri::menu::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu};
    use tterm_platform::tray::MenuEntry;
    entries
        .iter()
        .map(
            |entry| -> tauri::Result<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> {
                Ok(match entry {
                    MenuEntry::Separator => Box::new(PredefinedMenuItem::separator(app)?),
                    MenuEntry::Item {
                        id,
                        label,
                        enabled,
                        checked: Some(checked),
                    } => Box::new(CheckMenuItem::with_id(
                        app,
                        id,
                        label,
                        *enabled,
                        *checked,
                        None::<&str>,
                    )?),
                    MenuEntry::Item {
                        id,
                        label,
                        enabled,
                        checked: None,
                    } => Box::new(MenuItem::with_id(app, id, label, *enabled, None::<&str>)?),
                    MenuEntry::Submenu { label, entries } => {
                        let submenu = Submenu::new(app, label, true)?;
                        for item in menu_items(app, entries)? {
                            submenu.append(item.as_ref())?;
                        }
                        Box::new(submenu)
                    }
                })
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tterm_platform::tray::MenuEntry;

    fn model(label: &str) -> TrayModel {
        TrayModel { entries: vec![MenuEntry::Item {
            id: "show".into(), label: label.into(), enabled: true, checked: None,
        }] }
    }
    #[test]
    fn a_pending_removal_hides_presence_and_replays_only_the_latest_menu() {
        let mut state = RemovalState::default();
        assert!(state.is_present(true));
        assert!(state.begin_remove());
        assert!(!state.is_present(true));
        assert!(state.defer_present(model("first")).is_none());
        assert!(state.defer_present(model("second")).is_none());
        assert_eq!(state.deferred.take().unwrap().entries, model("second").entries);
        state.removing = false;
        assert!(state.defer_present(model("third")).is_some());
        assert!(state.is_present(true));
    }
    #[test]
    fn another_remove_cancels_a_queued_present_without_scheduling_two_drops() {
        let mut state = RemovalState::default();
        assert!(state.begin_remove());
        state.defer_present(model("tray"));
        assert!(!state.begin_remove());
        assert!(state.deferred.is_none());
        assert!(!state.is_present(true));
    }
}
