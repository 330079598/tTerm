//! Adapted from this repository's MIT implementation, not the nyakang/zed patch.
//! Source: tTerm, commit ede176759023f3f33af251815a7177d896bfa677,
//! src-tauri/src/drag_out/windows.rs (data_object and SHDoDragDrop flow).
//! Changes: GPUI owner validation; one known TEMP target instead of global Shell watching.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::Window;
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::Ole::{DROPEFFECT_COPY, DROPEFFECT_NONE};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    BHID_DataObject, ILFree, SHCreateShellItemArrayFromIDLists, SHDoDragDrop, SHParseDisplayName,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};
use windows::core::HSTRING;

use super::transfer::Folder;

/// Keeps the shell's empty placeholder alive throughout its modal drag loop.
pub struct Drag {
    pub folder: Arc<Folder>,
    pub accepted: bool,
    placeholder: PathBuf,
    copy: PathBuf,
}

impl Drag {
    pub fn prepare(target: &Path) -> Result<Self, String> {
        let folder = Folder::new()?;
        let name = format!(
            "tTerm-P0-19-{}.bin",
            folder
                .0
                .file_name()
                .ok_or("No folder name")?
                .to_string_lossy()
        );
        let placeholder = folder.0.join(&name);
        std::fs::write(&placeholder, []).map_err(|e| e.to_string())?;
        Ok(Self {
            folder,
            accepted: false,
            placeholder,
            copy: target.join(name),
        })
    }

    /// Uses the active GPUI window's HWND, checked against both process and title.
    /// The fixed dependency facade does not re-export raw-window-handle's trait.
    pub fn run(mut self, window: &Window) -> Result<Self, String> {
        if !window.is_window_active() {
            return Err("The GPUI probe must be active before dragging".into());
        }
        // SAFETY: querying a window handle, process id and title does not retain pointers.
        let owner = unsafe {
            let owner = GetForegroundWindow();
            let mut process = 0;
            GetWindowThreadProcessId(owner, Some(&mut process));
            let mut title = [0_u16; 256];
            let length = GetWindowTextW(owner, &mut title);
            if process != std::process::id()
                || String::from_utf16_lossy(&title[..length as usize]) != super::TITLE
            {
                return Err("Foreground HWND is not this GPUI probe; drag stopped".into());
            }
            owner
        };
        let data = data_object(&self.placeholder).map_err(|e| e.to_string())?;
        println!("native_drag_started hwnd={owner:?} placeholder_bytes=0");
        // SAFETY: GPUI initializes OLE on this thread. The owner and IDataObject
        // stay alive until Shell's modal loop returns; no GPUI internal drag is used.
        let effect = unsafe { SHDoDragDrop(Some(owner), &data, None, DROPEFFECT_COPY) }
            .map_err(|e| format!("Shell drag failed: {e}"))?;
        self.accepted = effect != DROPEFFECT_NONE;
        println!("native_drag_ended accepted={}", self.accepted);
        Ok(self)
    }

    pub fn has_copy(&self) -> bool {
        self.copy.is_file()
    }

    /// Only recognizes and removes this drag's unique empty placeholder in the
    /// explicitly permitted TEMP folder; unknown drop locations are never written.
    pub fn finish(&mut self) -> Result<bool, String> {
        match std::fs::metadata(&self.copy) {
            Ok(metadata) if metadata.len() == 0 && metadata.is_file() => {
                std::fs::remove_file(&self.copy).map_err(|e| e.to_string())?;
                Ok(true)
            }
            Ok(_) => Err("Drop placeholder changed; refusing to remove it".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }
}

pub(super) fn data_object(path: &Path) -> windows::core::Result<IDataObject> {
    let mut item = std::ptr::null_mut();
    // SAFETY: the live string is used during parsing. The returned PIDL is freed
    // exactly once after the shell has constructed its own data object.
    unsafe {
        SHParseDisplayName(&HSTRING::from(path.as_os_str()), None, &mut item, 0, None)?;
        let items: [*const ITEMIDLIST; 1] = [item.cast_const()];
        let result = SHCreateShellItemArrayFromIDLists(&items)
            .and_then(|items| items.BindToHandler(None, &BHID_DataObject));
        ILFree(Some(item));
        result
    }
}
