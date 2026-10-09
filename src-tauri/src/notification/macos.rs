//! Notifications through the User Notifications framework. It only works for
//! an app bundle (it throws for a bare binary), so a development build run
//! from `target/` shows them through AppleScript instead.

use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass};
use objc2_foundation::{
    ns_string, NSBundle, NSDictionary, NSError, NSObject, NSObjectProtocol, NSString,
};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSettings, UNNotificationSound, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};
use tauri::AppHandle;
use tokio::sync::oneshot;

use super::{NotificationPermission, NotificationRequest};

struct DelegateIvars {
    app: AppHandle,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and Delegate does not
    // implement Drop.
    #[unsafe(super(NSObject))]
    #[name = "TTermNotificationDelegate"]
    #[ivars = DelegateIvars]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        // Shown even with tTerm in front: the frontend only asks for one when
        // the window is not focused, which can still be the frontmost app.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            completion_handler.call((UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::List
                | UNNotificationPresentationOptions::Sound,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion_handler: &DynBlock<dyn Fn()>,
        ) {
            let user_info = response.notification().request().content().userInfo();
            // SAFETY: userInfo is a property list dictionary; its keys are strings.
            let user_info = unsafe { user_info.cast_unchecked::<NSString, AnyObject>() };
            let tab_id = user_info
                .objectForKey(ns_string!("tabId"))
                .and_then(|value| value.downcast::<NSString>().ok())
                .map(|value| value.to_string());
            super::activated(&self.ivars().app, tab_id);
            completion_handler.call(());
        }
    }
);

impl Delegate {
    fn new(app: AppHandle) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars { app });
        // SAFETY: NSObject's init.
        unsafe { msg_send![super(this), init] }
    }
}

fn is_app_bundle() -> bool {
    static BUNDLED: OnceLock<bool> = OnceLock::new();
    *BUNDLED.get_or_init(|| {
        NSBundle::mainBundle()
            .bundlePath()
            .to_string()
            .ends_with(".app")
    })
}

pub fn init(app: &AppHandle) {
    if !is_app_bundle() {
        return;
    }
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let delegate = Delegate::new(app.clone());
    center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    // The center holds its delegate weakly; this one lives as long as the app.
    std::mem::forget(delegate);
}

/// The outcome of one notification, sent once from whichever completion
/// handler finishes it.
type Reply = Arc<Mutex<Option<oneshot::Sender<Result<(), String>>>>>;

fn reply(reply: &Reply, result: Result<(), String>) {
    if let Some(sender) = reply.lock().ok().and_then(|mut sender| sender.take()) {
        let _ = sender.send(result);
    }
}

/// The framework's description of an error, when it passed one.
fn describe(error: *mut NSError) -> Option<String> {
    // SAFETY: the framework passes a valid error or null.
    unsafe { error.as_ref() }.map(|error| error.localizedDescription().to_string())
}

/// Resolves once the notification is delivered or refused, so a refusal
/// reaches the caller.
pub async fn show(request: NotificationRequest) -> Result<(), String> {
    if !is_app_bundle() {
        return show_with_applescript(request);
    }

    let (sender, receiver) = oneshot::channel();
    // In its own scope: blocks are not Send, so they must be gone before the await.
    {
        let pending: Reply = Arc::new(Mutex::new(Some(sender)));
        // Asks only the first time; later calls answer at once. The request is
        // added after the answer so the very first notification is not lost.
        let options = UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound;
        let handler = RcBlock::new(move |granted: Bool, error: *mut NSError| {
            if granted.as_bool() {
                add_request(&request, pending.clone());
                return;
            }
            let reason = describe(error).map(|reason| format!(" ({reason})"));
            reply(
                &pending,
                Err(format!(
                    "Notifications are not allowed for tTerm{}. Allow them in System Settings > Notifications.",
                    reason.unwrap_or_default()
                )),
            );
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .requestAuthorizationWithOptions_completionHandler(options, &handler);
    }
    receiver
        .await
        .unwrap_or_else(|_| Err("The notification was dropped".to_string()))
}

fn add_request(request: &NotificationRequest, pending: Reply) {
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(&request.title));
    if let Some(subtitle) = request.subtitle.as_deref().filter(|text| !text.is_empty()) {
        content.setSubtitle(&NSString::from_str(subtitle));
    }
    content.setBody(&NSString::from_str(&request.body));
    if request.sound {
        content.setSound(Some(&UNNotificationSound::defaultSound()));
    }
    if let Some(tab_id) = &request.tab_id {
        let tab_id = NSString::from_str(tab_id);
        // Notification Center groups a tab's notifications together.
        content.setThreadIdentifier(&tab_id);
        let user_info =
            NSDictionary::<NSString, NSString>::from_slices(&[ns_string!("tabId")], &[&*tab_id]);
        // SAFETY: a dictionary of strings is a valid property list.
        unsafe { content.setUserInfo(user_info.cast_unchecked::<AnyObject, AnyObject>()) };
    }

    let identifier = NSString::from_str(&uuid::Uuid::new_v4().to_string());
    let notification =
        UNNotificationRequest::requestWithIdentifier_content_trigger(&identifier, &content, None);
    let handler = RcBlock::new(move |error: *mut NSError| {
        let result = match describe(error) {
            Some(reason) => Err(format!("Failed to show a notification: {reason}")),
            None => Ok(()),
        };
        reply(&pending, result);
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .addNotificationRequest_withCompletionHandler(&notification, Some(&handler));
}

pub async fn permission() -> NotificationPermission {
    if !is_app_bundle() {
        return NotificationPermission::Fallback;
    }
    let (sender, receiver) = oneshot::channel();
    // In its own scope: the block is not Send, so it must be gone before the await.
    {
        let sender = Mutex::new(Some(sender));
        let handler = RcBlock::new(move |settings: std::ptr::NonNull<UNNotificationSettings>| {
            // SAFETY: the framework passes valid settings.
            let status = unsafe { settings.as_ref() }.authorizationStatus();
            if let Some(sender) = sender.lock().ok().and_then(|mut sender| sender.take()) {
                let _ = sender.send(status);
            }
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .getNotificationSettingsWithCompletionHandler(&handler);
    }
    match receiver.await {
        Ok(UNAuthorizationStatus::NotDetermined) => NotificationPermission::NotDetermined,
        Ok(UNAuthorizationStatus::Denied) => NotificationPermission::Denied,
        Ok(_) => NotificationPermission::Granted,
        Err(_) => NotificationPermission::NotDetermined,
    }
}

/// `display notification` for a build that is not an app bundle. It shows as
/// Script Editor's, and clicking it opens that, not tTerm.
fn show_with_applescript(request: NotificationRequest) -> Result<(), String> {
    fn quoted(text: &str) -> String {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
    }
    let mut script = format!(
        "display notification {} with title {}",
        quoted(&request.body),
        quoted(&request.title)
    );
    if let Some(subtitle) = request.subtitle.as_deref().filter(|text| !text.is_empty()) {
        script.push_str(&format!(" subtitle {}", quoted(subtitle)));
    }
    if request.sound {
        script.push_str(" sound name \"default\"");
    }
    let mut child = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .spawn()
        .map_err(|error| format!("Failed to run osascript: {error}"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
