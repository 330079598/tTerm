//! Re-attaches the Windows IME to the WebView after TSF loses track of it.
//!
//! Chromium binds a text field to TSF with `ITfThreadMgr::SetFocus` only. TSF
//! also re-targets its focused document on WM_ACTIVATE, and in WebView2 that
//! message reaches the host window in another process at no fixed point
//! relative to Chromium's own focus handling. When it lands last, TSF focuses
//! the window's default document instead: keys still reach the page, but the
//! IME composes with no text store, so the candidate window sits in the
//! screen's top-left corner and the result arrives as plain characters.
//!
//! Moving native focus to the host window and back makes Chromium run its
//! focus-in path again, which re-binds the text store. Unlike switching apps,
//! this does not re-activate the window, so it cannot lose the same race.

#[tauri::command]
pub fn repair_ime_focus(webview: tauri::Webview) -> Result<(), String> {
    #[cfg(windows)]
    webview
        .with_webview(|platform| {
            use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC;
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;

            let controller = platform.controller();
            unsafe {
                let mut parent = HWND::default();
                if controller.ParentWindow(&mut parent).is_ok() && !parent.is_invalid() {
                    let _ = SetFocus(Some(parent));
                }
                let _ = controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
            }
        })
        .map_err(|error| error.to_string())?;

    #[cfg(not(windows))]
    let _ = webview;

    Ok(())
}
