//! The operating system's own check that the person at the keyboard is the
//! signed-in user (Windows Hello: PIN, fingerprint or face; macOS: Touch ID
//! or the login password). Linux has none tTerm can rely on, so it falls back
//! to the master password.

#[cfg(target_os = "macos")]
mod platform {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_local_authentication::{LAContext, LAError, LAPolicy};
    use std::sync::mpsc;
    use tauri::AppHandle;

    /// Touch ID when the Mac has it set up, else the login password.
    const POLICY: LAPolicy = LAPolicy::DeviceOwnerAuthentication;

    pub(crate) fn available() -> bool {
        let context = unsafe { LAContext::new() };
        unsafe { context.canEvaluatePolicy_error(POLICY) }.is_ok()
    }

    /// Shows the system prompt and waits for it. Blocks, so it must not run
    /// on the main thread.
    pub(crate) fn verify(_app: &AppHandle, message: &str) -> Result<(), String> {
        let context = unsafe { LAContext::new() };
        let (sender, receiver) = mpsc::channel();
        let reply = RcBlock::new(move |success: Bool, error: *mut NSError| {
            let code = unsafe { error.as_ref() }.map(|error| error.code());
            let _ = sender.send((success.as_bool(), code));
        });
        unsafe {
            context.evaluatePolicy_localizedReason_reply(
                POLICY,
                &NSString::from_str(message),
                &reply,
            )
        };
        let (success, code) = receiver
            .recv()
            .map_err(|_| "Touch ID did not answer.".to_string())?;
        if success {
            return Ok(());
        }
        match code.map(LAError) {
            Some(LAError::UserCancel | LAError::SystemCancel | LAError::AppCancel) => {
                Err(super::CANCELED.to_string())
            }
            Some(LAError::PasscodeNotSet) => Err("This Mac has no login password set.".to_string()),
            Some(LAError::BiometryLockout) => {
                Err("Too many failed attempts. Try again later.".to_string())
            }
            _ => Err("macOS could not verify you.".to_string()),
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use tauri::{AppHandle, Manager};
    use windows::core::HSTRING;
    use windows::Security::Credentials::UI::{
        UserConsentVerificationResult, UserConsentVerifier, UserConsentVerifierAvailability,
    };
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::WinRT::IUserConsentVerifierInterop;
    use windows_future::IAsyncOperation;

    pub(crate) fn available() -> bool {
        UserConsentVerifier::CheckAvailabilityAsync()
            .and_then(|operation| operation.join())
            .is_ok_and(|availability| availability == UserConsentVerifierAvailability::Available)
    }

    /// Shows the Windows Hello prompt over the main window and waits for it.
    /// Blocks, so it must not run on the main thread.
    pub(crate) fn verify(app: &AppHandle, message: &str) -> Result<(), String> {
        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "The main window is not available.".to_string())?;
        // Tauri links another version of the `windows` crate; only the raw
        // handle is shared.
        let hwnd = HWND(
            window
                .hwnd()
                .map_err(|e| format!("Failed to get the window handle: {e}"))?
                .0,
        );
        let interop = windows::core::factory::<UserConsentVerifier, IUserConsentVerifierInterop>()
            .map_err(|e| format!("Windows Hello is unavailable: {e}"))?;
        let operation: IAsyncOperation<UserConsentVerificationResult> =
            unsafe { interop.RequestVerificationForWindowAsync(hwnd, &HSTRING::from(message)) }
                .map_err(|e| format!("Failed to start Windows Hello: {e}"))?;
        let result = operation
            .join()
            .map_err(|e| format!("Windows Hello failed: {e}"))?;
        match result {
            UserConsentVerificationResult::Verified => Ok(()),
            UserConsentVerificationResult::Canceled => Err(super::CANCELED.to_string()),
            UserConsentVerificationResult::DeviceNotPresent
            | UserConsentVerificationResult::NotConfiguredForUser
            | UserConsentVerificationResult::DisabledByPolicy => {
                Err("Windows Hello is not set up on this device.".to_string())
            }
            UserConsentVerificationResult::DeviceBusy => {
                Err("Windows Hello is busy. Try again.".to_string())
            }
            UserConsentVerificationResult::RetriesExhausted => {
                Err("Too many failed attempts. Try again later.".to_string())
            }
            _ => Err("Windows Hello could not verify you.".to_string()),
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod platform {
    use tauri::AppHandle;

    pub(crate) fn available() -> bool {
        false
    }

    pub(crate) fn verify(_app: &AppHandle, _message: &str) -> Result<(), String> {
        Err("System verification is not supported on this platform.".to_string())
    }
}

pub(crate) const CANCELED: &str = "Verification was canceled.";

pub(crate) use platform::{available, verify};
