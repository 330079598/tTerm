//! Dragging out on Windows. File Explorer never tells a drag source where a
//! drop landed, so tTerm drags empty placeholders with unique names and
//! listens for the shell's notice that they were created; the folder they
//! appeared in is the target. The copied placeholders are then deleted.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once};
use std::time::Duration;

use tauri::WebviewWindow;
use tokio::sync::oneshot;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::{DROPEFFECT_COPY, DROPEFFECT_NONE};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    BHID_DataObject, ILFree, SHCNRF_NewDelivery, SHCNRF_ShellLevel, SHChangeNotification_Lock,
    SHChangeNotification_Unlock, SHChangeNotify, SHChangeNotifyDeregister, SHChangeNotifyEntry,
    SHChangeNotifyRegister, SHCreateShellItemArrayFromIDLists, SHDoDragDrop, SHGetPathFromIDListW,
    SHGetSpecialFolderLocation, SHParseDisplayName, CSIDL_DESKTOP, SHCNE_CREATE, SHCNE_DELETE,
    SHCNE_ID, SHCNE_MKDIR, SHCNE_RMDIR, SHCNE_UPDATEDIR, SHCNF_FLUSHNOWAIT, SHCNF_PATHW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetAncestor, GetCursorPos, RegisterClassW,
    WindowFromPoint, GA_ROOT, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WNDCLASSW,
};

use super::{DragOutItem, DragOutResult};
use crate::core::blocking::run_blocking;

const WATCHER_CLASS: PCWSTR = w!("tTermDragOutWatcher");
const WM_SHELL_CHANGE: u32 = WM_APP + 0x31;

/// How long to wait for File Explorer to copy the first placeholder once it
/// took the drop. Copying an empty file is instant on a local disk; the
/// margin is for network folders.
const COPY_TIMEOUT: Duration = Duration::from_secs(10);
/// A cancelled drag can still have been dropped: some targets report no
/// effect even though they copied the files.
const CANCELLED_GRACE: Duration = Duration::from_millis(400);
/// How long the remaining placeholders get to arrive after the first, so
/// that all of them are found and deleted.
const REST_TIMEOUT: Duration = Duration::from_secs(2);

/// Extensions whose empty files the shell would try to parse for the drag
/// image (shortcuts and the like); their placeholders go without one.
const PARSED_EXTENSIONS: &[&str] = &["lnk", "url", "pif", "scf"];

/// Placeholders still awaited by one watcher window.
struct Pending {
    watcher: isize,
    names: Vec<OsString>,
    copies: Vec<PathBuf>,
    first: Option<oneshot::Sender<()>>,
}

/// Placeholders still awaited. The watcher's window procedure runs on the
/// main thread while the async side reads the copies found so far.
static PENDING: Mutex<Vec<Pending>> = Mutex::new(Vec::new());

/// A message-only window registered for the shell's create notices. Plain
/// numbers so it can cross threads; it is only used on the main thread.
struct Watcher {
    hwnd: isize,
    registration: u32,
}

impl Watcher {
    fn stop(self) {
        // SAFETY: both were created by `start_watcher` on this thread.
        unsafe {
            let _ = SHChangeNotifyDeregister(self.registration);
            let _ = DestroyWindow(HWND(self.hwnd as _));
        }
    }
}

/// What the drag on the main thread reports back.
struct DragEnd {
    watcher: Watcher,
    /// A target other than tTerm took the drop.
    dropped_elsewhere: bool,
    /// Let go over tTerm's own window, which accepts files to upload.
    over_self: bool,
}

pub async fn drag_out(
    window: WebviewWindow,
    items: Vec<DragOutItem>,
) -> Result<DragOutResult, String> {
    let token = uuid::Uuid::new_v4().simple().to_string();
    let folder = std::env::temp_dir().join(format!("tTerm-drag-{token}"));
    let placeholders = {
        let folder = folder.clone();
        run_blocking(move || create_placeholders(&folder, &items, &token)).await
    };
    let result = match placeholders {
        Ok(placeholders) => drag_placeholders(&window, placeholders).await,
        Err(error) => Err(error),
    };
    let _ = run_blocking(move || {
        let _ = std::fs::remove_dir_all(&folder);
        Ok(())
    })
    .await;
    result
}

/// A unique, Windows-safe name for the placeholder of `item`. A file keeps
/// its extension so the drag shows its type's icon.
fn placeholder_name(item: &DragOutItem, index: usize, token: &str) -> String {
    let base = format!("tTerm-drop-{token}-{index}");
    if item.is_dir {
        return base;
    }
    let extension = Path::new(&item.name)
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| {
            !extension.is_empty()
                && extension.len() <= 16
                && extension.chars().all(|c| c.is_ascii_alphanumeric())
                && !PARSED_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        });
    match extension {
        Some(extension) => format!("{base}.{extension}"),
        None => base,
    }
}

fn create_placeholders(
    folder: &Path,
    items: &[DragOutItem],
    token: &str,
) -> Result<Vec<PathBuf>, String> {
    let fail =
        |e: std::io::Error| format!("Failed to prepare the drag in the temporary folder: {e}");
    std::fs::create_dir_all(folder).map_err(fail)?;
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let path = folder.join(placeholder_name(item, index, token));
            if item.is_dir {
                std::fs::create_dir(&path).map_err(fail)?;
            } else {
                std::fs::write(&path, b"").map_err(fail)?;
            }
            Ok(path)
        })
        .collect()
}

async fn drag_placeholders(
    window: &WebviewWindow,
    placeholders: Vec<PathBuf>,
) -> Result<DragOutResult, String> {
    let (first_sender, first) = oneshot::channel();
    let (ended_sender, ended) = oneshot::channel();
    let target = window.clone();
    let expected = placeholders.len();
    window
        .run_on_main_thread(move || {
            let _ = ended_sender.send(run_drag(&target, &placeholders, first_sender));
        })
        .map_err(|e| format!("Failed to start the drag: {e}"))?;
    let end = ended
        .await
        .map_err(|_| "The drag was interrupted".to_string())??;

    let wait = if end.over_self {
        Duration::ZERO
    } else if end.dropped_elsewhere {
        COPY_TIMEOUT
    } else {
        CANCELLED_GRACE
    };
    let arrived = matches!(tokio::time::timeout(wait, first).await, Ok(Ok(())));
    if arrived {
        let deadline = tokio::time::Instant::now() + REST_TIMEOUT;
        while copies_found(end.watcher.hwnd) < expected && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    let copies = take_copies(end.watcher.hwnd);
    let watcher = end.watcher;
    let _ = window.run_on_main_thread(move || watcher.stop());

    let Some(directory) = copies
        .first()
        .and_then(|copy| copy.parent())
        .map(|parent| parent.to_string_lossy().into_owned())
    else {
        return Ok(if end.dropped_elsewhere && !end.over_self {
            DragOutResult::UnknownTarget
        } else {
            DragOutResult::Cancelled
        });
    };
    run_blocking(move || {
        copies
            .iter()
            .for_each(|copy| remove_copied_placeholder(copy));
        Ok(())
    })
    .await?;
    Ok(DragOutResult::Dropped { directory })
}

fn copies_found(watcher: isize) -> usize {
    PENDING.lock().map_or(0, |pending| {
        pending
            .iter()
            .find(|item| item.watcher == watcher)
            .map_or(0, |item| item.copies.len())
    })
}

fn take_copies(watcher: isize) -> Vec<PathBuf> {
    let Ok(mut pending) = PENDING.lock() else {
        return Vec::new();
    };
    let Some(index) = pending.iter().position(|item| item.watcher == watcher) else {
        return Vec::new();
    };
    pending.swap_remove(index).copies
}

/// File Explorer can still hold a new file open for a moment after its
/// notice, so the delete is retried briefly.
fn remove_copied_placeholder(copy: &Path) {
    let is_dir = copy.is_dir();
    for _ in 0..10 {
        let removed = if is_dir {
            std::fs::remove_dir(copy)
        } else {
            std::fs::remove_file(copy)
        };
        match removed {
            Ok(()) => {
                // File Explorer added the placeholder to its view itself while
                // copying it, and can miss the file system's notice of a
                // delete that follows so soon; the shell's own notice removes
                // it from the view instead of leaving it until a refresh.
                notify_shell(if is_dir { SHCNE_RMDIR } else { SHCNE_DELETE }, copy);
                return;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    eprintln!(
        "[drag_out] Failed to delete the drop placeholder {}",
        copy.display()
    );
}

/// Tells File Explorer to list `directory` again, as a refresh would. Right
/// after a drop it can miss the file system's notices for the files the
/// download writes there, renames and deletes.
pub fn refresh_folder(directory: &Path) {
    notify_shell(SHCNE_UPDATEDIR, directory);
}

fn notify_shell(event: SHCNE_ID, path: &Path) {
    let path = HSTRING::from(path.as_os_str());
    // SAFETY: the path is a live, nul-terminated string, and the shell reads
    // it during the call. FLUSHNOWAIT delivers the notice without waiting
    // for File Explorer to handle it.
    unsafe {
        SHChangeNotify(
            event,
            SHCNF_PATHW | SHCNF_FLUSHNOWAIT,
            Some(path.as_ptr().cast()),
            None,
        );
    }
}

/// Runs on the main thread, which OLE has initialized for drag and drop.
/// Blocks in the system's drag loop until the mouse button is released.
fn run_drag(
    window: &WebviewWindow,
    placeholders: &[PathBuf],
    first: oneshot::Sender<()>,
) -> Result<DragEnd, String> {
    // Tauri links another version of the `windows` crate; only the raw
    // handle is shared.
    let hwnd = HWND(
        window
            .hwnd()
            .map_err(|e| format!("Failed to get the window handle: {e}"))?
            .0,
    );
    let data = data_object(placeholders).map_err(|e| format!("Failed to start the drag: {e}"))?;
    let names = placeholders
        .iter()
        .filter_map(|path| path.file_name().map(ToOwned::to_owned))
        .collect();
    let watcher = start_watcher(names, first)?;
    // SAFETY: the data object is a live COM object owned by this frame.
    let effect = match unsafe { SHDoDragDrop(Some(hwnd), &data, None, DROPEFFECT_COPY) } {
        Ok(effect) => effect,
        Err(error) => {
            take_copies(watcher.hwnd);
            watcher.stop();
            return Err(format!("Failed to start the drag: {error}"));
        }
    };

    Ok(DragEnd {
        watcher,
        dropped_elsewhere: effect != DROPEFFECT_NONE,
        over_self: cursor_over(hwnd),
    })
}

/// The shell's data object for the placeholders, which all share a folder.
/// It carries the formats File Explorer reads and gives the drag its image.
fn data_object(placeholders: &[PathBuf]) -> windows::core::Result<IDataObject> {
    let mut items: Vec<*const ITEMIDLIST> = Vec::with_capacity(placeholders.len());
    let result = (|| {
        for path in placeholders {
            let mut item = std::ptr::null_mut();
            // SAFETY: the path is a live string and `item` receives a new ID
            // list, freed below.
            unsafe {
                SHParseDisplayName(&HSTRING::from(path.as_os_str()), None, &mut item, 0, None)?;
            }
            items.push(item.cast_const());
        }
        // SAFETY: every ID list in `items` is valid until freed below.
        unsafe { SHCreateShellItemArrayFromIDLists(&items)?.BindToHandler(None, &BHID_DataObject) }
    })();
    for item in items {
        // SAFETY: each ID list came from SHParseDisplayName and is freed once.
        unsafe { ILFree(Some(item)) };
    }
    result
}

fn cursor_over(hwnd: HWND) -> bool {
    let mut point = POINT::default();
    // SAFETY: `point` outlives the call.
    unsafe {
        GetCursorPos(&mut point).is_ok() && GetAncestor(WindowFromPoint(point), GA_ROOT) == hwnd
    }
}

fn start_watcher(names: Vec<OsString>, first: oneshot::Sender<()>) -> Result<Watcher, String> {
    static REGISTER_CLASS: Once = Once::new();
    let fail = |error: windows::core::Error| format!("Failed to watch for the drop: {error}");

    // SAFETY: plain Win32 calls; the class and window live as long as tTerm
    // or until `Watcher::stop`, and the notify entry outlives the register call.
    unsafe {
        let instance = GetModuleHandleW(None).map_err(fail)?;
        REGISTER_CLASS.call_once(|| {
            let class = WNDCLASSW {
                lpfnWndProc: Some(watcher_proc),
                hInstance: instance.into(),
                lpszClassName: WATCHER_CLASS,
                ..Default::default()
            };
            RegisterClassW(&class);
        });
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            WATCHER_CLASS,
            None,
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance.into()),
            None,
        )
        .map_err(fail)?;

        // The desktop is the root of the shell namespace, so a recursive
        // watch on it hears about every folder, the desktop included.
        let root = match SHGetSpecialFolderLocation(None, CSIDL_DESKTOP as i32) {
            Ok(root) => root,
            Err(error) => {
                let _ = DestroyWindow(hwnd);
                return Err(fail(error));
            }
        };
        let entry = SHChangeNotifyEntry {
            pidl: root,
            fRecursive: true.into(),
        };
        let registration = SHChangeNotifyRegister(
            hwnd,
            SHCNRF_ShellLevel | SHCNRF_NewDelivery,
            (SHCNE_CREATE.0 | SHCNE_MKDIR.0) as i32,
            WM_SHELL_CHANGE,
            1,
            &entry,
        );
        ILFree(Some(root.cast_const()));
        if registration == 0 {
            let _ = DestroyWindow(hwnd);
            return Err("Failed to watch for the drop: the shell refused the request".to_string());
        }

        if let Ok(mut pending) = PENDING.lock() {
            pending.push(Pending {
                watcher: hwnd.0 as isize,
                names,
                copies: Vec::new(),
                first: Some(first),
            });
        }
        Ok(Watcher {
            hwnd: hwnd.0 as isize,
            registration,
        })
    }
}

unsafe extern "system" fn watcher_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message != WM_SHELL_CHANGE {
        // SAFETY: forwards the window procedure's own arguments.
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    // SAFETY: with SHCNRF_NewDelivery the parameters are the change handle
    // and process id that SHChangeNotification_Lock expects.
    if let Some(path) = unsafe { notified_path(wparam, lparam) } {
        record_copy(hwnd.0 as isize, path);
    }
    LRESULT(0)
}

/// The path of the item a shell create notice is about.
unsafe fn notified_path(wparam: WPARAM, lparam: LPARAM) -> Option<PathBuf> {
    let mut items: *mut *mut ITEMIDLIST = std::ptr::null_mut();
    let mut event = 0i32;
    // SAFETY: the caller passes a notice's parameters; the lock is released
    // below, after the item has been read.
    unsafe {
        let lock = SHChangeNotification_Lock(
            HANDLE(wparam.0 as _),
            lparam.0 as u32,
            Some(&mut items),
            Some(&mut event),
        );
        if lock.is_invalid() {
            return None;
        }
        let mut buffer = [0u16; 260];
        let found = !items.is_null()
            && !(*items).is_null()
            && SHGetPathFromIDListW(*items, &mut buffer).as_bool();
        let _ = SHChangeNotification_Unlock(lock);
        if !found {
            return None;
        }
        let length = buffer.iter().position(|unit| *unit == 0)?;
        Some(PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
    }
}

fn record_copy(watcher: isize, path: PathBuf) {
    let Ok(mut pending) = PENDING.lock() else {
        return;
    };
    let Some(item) = pending.iter_mut().find(|item| item.watcher == watcher) else {
        return;
    };
    let Some(name) = path.file_name() else {
        return;
    };
    // The shell can send the same notice more than once.
    if !item.names.iter().any(|expected| expected == name) || item.copies.contains(&path) {
        return;
    }
    item.copies.push(path);
    if let Some(first) = item.first.take() {
        let _ = first.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, is_dir: bool) -> DragOutItem {
        DragOutItem {
            name: name.to_string(),
            is_dir,
        }
    }

    #[test]
    fn placeholder_name_keeps_plain_extensions_only() {
        assert_eq!(
            placeholder_name(&item("report.PDF", false), 0, "abc"),
            "tTerm-drop-abc-0.PDF"
        );
        assert_eq!(
            placeholder_name(&item("archive.tar.gz", false), 1, "abc"),
            "tTerm-drop-abc-1.gz"
        );
        assert_eq!(
            placeholder_name(&item("Makefile", false), 2, "abc"),
            "tTerm-drop-abc-2"
        );
        assert_eq!(
            placeholder_name(&item("odd.e:x", false), 3, "abc"),
            "tTerm-drop-abc-3"
        );
        assert_eq!(
            placeholder_name(&item("home.LNK", false), 4, "abc"),
            "tTerm-drop-abc-4"
        );
        assert_eq!(
            placeholder_name(&item("photos.d", true), 5, "abc"),
            "tTerm-drop-abc-5"
        );
    }
}
