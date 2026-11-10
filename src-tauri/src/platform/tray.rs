use tauri::AppHandle;
use tterm_platform::tray::{Tray, TrayModel};
pub(super) struct TauriTray(pub AppHandle);
impl Tray for TauriTray {
    fn present(&self, model: TrayModel) -> Result<(), String> {
        let menu = build_menu(&self.0, &model.entries).map_err(|e| e.to_string())?;
        crate::background::tray::present_menu(&self.0, menu).map_err(|e| e.to_string())
    }
    fn remove(&self) -> Result<(), String> {
        // Dropping the last macOS status-item reference must happen on the UI
        // thread, including when save_config called this from a blocking worker.
        let app = self.0.clone();
        self.0
            .run_on_main_thread(move || crate::background::tray::hide(&app))
            .map_err(|error| error.to_string())
    }
    fn is_present(&self) -> bool {
        self.0
            .tray_by_id(crate::background::tray::TRAY_ID)
            .is_some()
    }
    fn set_tooltip(&self, text: String) -> Result<(), String> {
        if let Some(tray) = self.0.tray_by_id(crate::background::tray::TRAY_ID) {
            tray.set_tooltip(Some(text)).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
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
