//! Platform capabilities that the runtime prototype has not wired to GPUI yet.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tterm_core::identity::AppIdentity;
use tterm_core::services::shared::PlatformServices;
use tterm_platform::PlatformFuture;
use tterm_platform::clipboard::Clipboard;
use tterm_platform::drag::{DragOutItem, DragOutResult, FileDrag};
use tterm_platform::native::notification::NativeNotifications;
use tterm_platform::notifications::{AppBadge, Notifications, RgbaImage};
use tterm_platform::open::{ApplicationChoice, OpenTargets};
use tterm_platform::process::ProcessControl;
use tterm_platform::tray::{Tray, TrayModel};
use tterm_platform::updates::{AvailableUpdate, Updates};
use tterm_platform::verification::UserVerification;
use tterm_platform::window::MainWindow;

/// Builds adapters around capabilities that exist independently of a UI toolkit.
pub(super) fn services(identity: &AppIdentity) -> PlatformServices {
    let notifications: Arc<dyn Notifications> = Arc::new(NativeNotifications {
        identifier: identity.identifier.clone(),
        activated: Arc::new(|_| {}),
    });
    let unavailable = Arc::new(Unavailable);
    PlatformServices {
        notifications,
        integration: Err("System integration is not wired to GPUI yet".to_string()),
        badge: unavailable.clone(),
        tray: unavailable.clone(),
        file_drag: unavailable.clone(),
        main_window: unavailable.clone(),
        process: unavailable.clone(),
        verification: unavailable.clone(),
        updates: unavailable.clone(),
        open: unavailable.clone(),
        clipboard: unavailable,
    }
}

struct Unavailable;

impl AppBadge for Unavailable {
    fn set_unseen(&self, _: u32, _: Option<RgbaImage>) -> PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn restore(&self) -> Result<(), String> {
        Ok(())
    }
}

impl Tray for Unavailable {
    fn dispatch(&self, task: Box<dyn FnOnce() + Send>) -> Result<(), String> {
        task();
        Ok(())
    }

    fn present(&self, _: TrayModel) -> Result<(), String> {
        Err("The tray is not wired to GPUI yet".to_string())
    }

    fn remove(&self) -> Result<(), String> {
        Ok(())
    }

    fn is_present(&self) -> bool {
        false
    }

    fn set_tooltip(&self, _: String) -> Result<(), String> {
        Ok(())
    }
}

impl MainWindow for Unavailable {
    fn save_before_update(&self) -> PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn send_to_background(&self, _: bool) -> Result<(), String> {
        Ok(())
    }

    fn show_and_focus(&self) -> Result<(), String> {
        Ok(())
    }

    fn reveal_for_prompt(&self) -> Result<(), String> {
        Ok(())
    }

    fn set_attention(&self, _: bool) -> Result<(), String> {
        Ok(())
    }
}

impl ProcessControl for Unavailable {
    fn exit(&self, code: i32) -> Result<(), String> {
        std::process::exit(code);
    }

    fn restart(&self) -> Result<(), String> {
        Err("Restart is not wired to GPUI yet".to_string())
    }
}

impl UserVerification for Unavailable {
    fn available(&self) -> bool {
        false
    }

    fn verify(&self, _: &str) -> Result<(), String> {
        Err("System verification is not wired to GPUI yet".to_string())
    }
}

impl Updates for Unavailable {
    fn check(&self, _: String) -> PlatformFuture<'_, Option<Box<dyn AvailableUpdate>>> {
        Box::pin(async { Ok(None) })
    }
}

impl FileDrag for Unavailable {
    fn start(&self, _: Vec<DragOutItem>) -> PlatformFuture<'_, DragOutResult> {
        Box::pin(async { Ok(DragOutResult::UnknownTarget) })
    }

    fn refresh_folder(&self, _: PathBuf) -> PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

impl Clipboard for Unavailable {
    fn read_text(&self) -> PlatformFuture<'_, String> {
        Box::pin(async { Ok(String::new()) })
    }

    fn write_text(&self, _: String) -> PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn read_files(&self) -> PlatformFuture<'_, Vec<PathBuf>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

impl OpenTargets for Unavailable {
    fn open_url(&self, _: String) -> PlatformFuture<'_, ()> {
        Box::pin(async { Err("Opening URLs is not wired to GPUI yet".to_string()) })
    }

    fn open_file(&self, _: PathBuf) -> PlatformFuture<'_, ()> {
        Box::pin(async { Err("Opening files is not wired to GPUI yet".to_string()) })
    }

    fn reveal(&self, _: PathBuf) -> PlatformFuture<'_, ()> {
        Box::pin(async { Err("Revealing files is not wired to GPUI yet".to_string()) })
    }

    fn open_with(&self, _: String, _: PathBuf) -> PlatformFuture<'_, ()> {
        Box::pin(async { Err("Opening files is not wired to GPUI yet".to_string()) })
    }

    fn choose_application(&self, _: PathBuf) -> PlatformFuture<'_, bool> {
        Box::pin(async { Ok(false) })
    }

    fn applications_for(&self, _: String) -> PlatformFuture<'_, Vec<ApplicationChoice>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn application_icons(
        &self,
        _: Vec<String>,
    ) -> PlatformFuture<'_, HashMap<String, Option<Vec<u8>>>> {
        Box::pin(async { Ok(HashMap::new()) })
    }
}
