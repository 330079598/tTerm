//! The applications macOS offers for a kind of file, as Finder's Open With
//! menu lists them, and their icons. Drawing the icons of a dozen
//! applications takes a second the first time, so the list comes with the
//! icons drawn before and the rest are asked for after it shows.

use super::AppChoice;
use base64::Engine;
use objc2::AllocAnyThread;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace,
    NSGraphicsContext, NSImage, NSWorkspace,
};
use objc2_foundation::{NSDictionary, NSFileManager, NSPoint, NSRect, NSSize, NSString, NSURL};
use objc2_uniform_type_identifiers::UTType;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};

/// The side of an icon in pixels: 32 points, drawn for a Retina screen.
const ICON_PIXELS: isize = 64;

/// Icons drawn so far, by application path: drawing one takes tens of
/// milliseconds, and a kind of file can have a dozen applications.
static ICONS: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn icons() -> std::sync::MutexGuard<'static, HashMap<String, Option<String>>> {
    ICONS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The icons of the applications at `paths`, drawing those not drawn yet.
pub(super) fn icons_for(paths: Vec<String>) -> HashMap<String, Option<String>> {
    let workspace = NSWorkspace::sharedWorkspace();
    paths
        .into_iter()
        .map(|path| {
            let cached = icons().get(&path).cloned();
            let icon = cached.unwrap_or_else(|| {
                let icon = icon_data_url(&workspace.iconForFile(&NSString::from_str(&path)));
                icons().insert(path.clone(), icon.clone());
                icon
            });
            (path, icon)
        })
        .collect()
}

/// The kind of file the name says. A name without an extension, such as
/// `Makefile` or `config`, is most likely text on a server.
fn content_type(file_name: &str) -> Option<objc2::rc::Retained<UTType>> {
    std::path::Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| UTType::typeWithFilenameExtension(&NSString::from_str(extension)))
        .or_else(|| UTType::typeWithIdentifier(&NSString::from_str("public.plain-text")))
}

fn path_of(url: &NSURL) -> Option<String> {
    url.path().map(|path| path.to_string())
}

/// The icon as a PNG data URL, or None when it cannot be drawn.
fn icon_data_url(image: &NSImage) -> Option<String> {
    // SAFETY: a null `planes` asks the rep to allocate its own buffer; the
    // other arguments describe an ordinary 8-bit RGBA bitmap.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            ICON_PIXELS,
            ICON_PIXELS,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )
    }?;
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
    let side = ICON_PIXELS as f64;
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    image.drawInRect_fromRect_operation_fraction(
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(side, side)),
        NSRect::ZERO,
        NSCompositingOperation::Copy,
        1.0,
    );
    context.flushGraphics();
    NSGraphicsContext::restoreGraphicsState_class();
    // SAFETY: an empty dictionary is a valid set of PNG properties.
    let png = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png.to_vec())
    ))
}

/// The applications that open `file_name`, its default one first, then by
/// name, with the icons drawn so far.
pub(super) fn apps_for(file_name: &str) -> Vec<AppChoice> {
    let Some(content_type) = content_type(file_name) else {
        return Vec::new();
    };
    let workspace = NSWorkspace::sharedWorkspace();
    let files = NSFileManager::defaultManager();
    let default = workspace
        .URLForApplicationToOpenContentType(&content_type)
        .and_then(|url| path_of(&url));
    let urls = workspace.URLsForApplicationsToOpenContentType(&content_type);
    let mut apps: Vec<AppChoice> = Vec::new();
    for url in urls.iter() {
        let Some(path) = path_of(&url) else {
            continue;
        };
        if apps.iter().any(|app| app.path == path) {
            continue;
        }
        let ns_path = NSString::from_str(&path);
        apps.push(AppChoice {
            name: files.displayNameAtPath(&ns_path).to_string(),
            is_default: default.as_deref() == Some(path.as_str()),
            icon: icons().get(&path).cloned().flatten(),
            path,
        });
    }
    apps.sort_by(|a, b| {
        b.is_default
            .cmp(&a.is_default)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    apps
}
