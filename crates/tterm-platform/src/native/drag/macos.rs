//! Dragging out on macOS through file promises. Finder asks the promise for
//! its file only after the drop, naming the folder the file goes in; that
//! folder is the target, and the frontend downloads into it.

use std::cell::RefCell;
use std::ffi::c_void;
use std::time::Duration;

use block2::DynBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{
    define_class, msg_send, AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSDragOperation, NSDraggingContext, NSDraggingItem, NSDraggingSession, NSDraggingSource,
    NSEvent, NSEventModifierFlags, NSEventType, NSFilePromiseProvider,
    NSFilePromiseProviderDelegate, NSImage, NSView, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSError, NSNumber, NSObject, NSObjectProtocol, NSOperationQueue, NSPoint,
    NSProcessInfo, NSRect, NSSize, NSString, NSURL,
};
use objc2_uniform_type_identifiers::UTType;
use tauri::WebviewWindow;
use tokio::sync::{mpsc, oneshot};

use super::{DragOutItem, DragOutResult};

/// How long Finder gets to ask for the files once it took the drop.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// The side of the icon shown for each dragged item.
const ICON_SIZE: f64 = 48.0;

/// What the drag reports while it runs.
#[derive(Debug)]
enum DragEvent {
    Ended { dropped: bool },
    Requested { directory: String },
}

struct SourceIvars {
    names: Vec<String>,
    events: mpsc::UnboundedSender<DragEvent>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and DragOutSource
    // does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "TTermDragOutSource"]
    #[ivars = SourceIvars]
    struct DragOutSource;

    unsafe impl NSObjectProtocol for DragOutSource {}

    unsafe impl NSDraggingSource for DragOutSource {
        // Nothing inside tTerm takes the files, and the window's own file
        // drop would read the drag as an upload.
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn operation_mask(
            &self,
            _session: &NSDraggingSession,
            context: NSDraggingContext,
        ) -> NSDragOperation {
            if context == NSDraggingContext::OutsideApplication {
                NSDragOperation::Copy
            } else {
                NSDragOperation::None
            }
        }

        #[unsafe(method(draggingSession:endedAtPoint:operation:))]
        fn ended(&self, _session: &NSDraggingSession, _point: NSPoint, operation: NSDragOperation) {
            let _ = self.ivars().events.send(DragEvent::Ended {
                dropped: operation != NSDragOperation::None,
            });
        }
    }

    unsafe impl NSFilePromiseProviderDelegate for DragOutSource {
        #[unsafe(method_id(filePromiseProvider:fileNameForType:))]
        fn file_name(
            &self,
            provider: &NSFilePromiseProvider,
            _file_type: &NSString,
        ) -> Retained<NSString> {
            let index = provider
                .userInfo()
                .and_then(|info| info.downcast::<NSNumber>().ok())
                .map(|index| index.as_usize());
            let name = index
                .and_then(|index| self.ivars().names.get(index))
                .map_or("download", String::as_str);
            NSString::from_str(name)
        }

        // The file itself is downloaded by the frontend afterwards, through
        // the transfer list, so the promise is reported kept right away.
        #[unsafe(method(filePromiseProvider:writePromiseToURL:completionHandler:))]
        fn write_promise(
            &self,
            _provider: &NSFilePromiseProvider,
            url: &NSURL,
            completion_handler: &DynBlock<dyn Fn(*mut NSError)>,
        ) {
            let directory = url
                .URLByDeletingLastPathComponent()
                .and_then(|folder| folder.path())
                .map(|path| path.to_string());
            if let Some(directory) = directory {
                let _ = self.ivars().events.send(DragEvent::Requested { directory });
            }
            completion_handler.call((std::ptr::null_mut(),));
        }

        // Answered on the main thread, where this source lives.
        #[unsafe(method_id(operationQueueForFilePromiseProvider:))]
        fn operation_queue(&self, _provider: &NSFilePromiseProvider) -> Retained<NSOperationQueue> {
            NSOperationQueue::mainQueue()
        }
    }
);

impl DragOutSource {
    fn new(
        mtm: MainThreadMarker,
        names: Vec<String>,
        events: mpsc::UnboundedSender<DragEvent>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(SourceIvars { names, events });
        // SAFETY: NSObject's init.
        unsafe { msg_send![super(this), init] }
    }
}

thread_local! {
    /// The source of the latest drag. Promise providers hold their delegate
    /// weakly, and Finder can ask for the files after the drag ended.
    static SOURCE: RefCell<Option<Retained<DragOutSource>>> = const { RefCell::new(None) };
}

pub async fn drag_out(
    window: WebviewWindow,
    items: Vec<DragOutItem>,
) -> Result<DragOutResult, String> {
    let (events_sender, events) = mpsc::unbounded_channel();
    let (started_sender, started) = oneshot::channel();
    let target = window.clone();
    window
        .run_on_main_thread(move || {
            let result = target
                .ns_view()
                .map_err(|e| format!("Failed to get the window's view: {e}"))
                .and_then(|view| begin_drag(view, &items, events_sender));
            let _ = started_sender.send(result);
        })
        .map_err(|e| format!("Failed to start the drag: {e}"))?;
    started
        .await
        .map_err(|_| "The drag was interrupted".to_string())??;
    Ok(wait_for_drop(events).await)
}

async fn wait_for_drop(mut events: mpsc::UnboundedReceiver<DragEvent>) -> DragOutResult {
    let mut dropped = false;
    loop {
        let event = if dropped {
            tokio::time::timeout(WRITE_TIMEOUT, events.recv())
                .await
                .ok()
                .flatten()
        } else {
            events.recv().await
        };
        match event {
            Some(DragEvent::Requested { directory }) => {
                return DragOutResult::Dropped { directory };
            }
            Some(DragEvent::Ended { dropped: true }) => dropped = true,
            Some(DragEvent::Ended { dropped: false }) => return DragOutResult::Cancelled,
            None if dropped => return DragOutResult::UnknownTarget,
            None => return DragOutResult::Cancelled,
        }
    }
}

/// The uniform type of an item, which Finder uses for its promise.
fn content_type(item: &DragOutItem) -> Option<Retained<UTType>> {
    if item.is_dir {
        return UTType::typeWithIdentifier(&NSString::from_str("public.folder"));
    }
    std::path::Path::new(&item.name)
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| UTType::typeWithFilenameExtension(&NSString::from_str(extension)))
        .or_else(|| UTType::typeWithIdentifier(&NSString::from_str("public.data")))
}

fn icon(content_type: Option<&UTType>) -> Option<Retained<NSImage>> {
    content_type.map(|content_type| NSWorkspace::sharedWorkspace().iconForContentType(content_type))
}

/// Runs on the main thread while the mouse button that started the drag is
/// still held; AppKit then tracks the drag on its own.
fn begin_drag(
    view: *mut c_void,
    items: &[DragOutItem],
    events: mpsc::UnboundedSender<DragEvent>,
) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("The drag must start on the main thread")?;
    // SAFETY: Tauri hands out the window's content view, which lives as long
    // as the window, and this runs on the main thread that owns it.
    let view = unsafe { view.cast::<NSView>().as_ref() }.ok_or("The window has no view")?;
    let window = view.window().ok_or("The view is not in a window")?;
    let in_window = window.mouseLocationOutsideOfEventStream();
    let point = view.convertPoint_fromView(in_window, None);

    let source = DragOutSource::new(
        mtm,
        items.iter().map(|item| item.name.clone()).collect(),
        events,
    );
    let dragging_items: Vec<Retained<NSDraggingItem>> = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let content_type = content_type(item);
            let type_identifier = content_type.as_deref().map_or_else(
                || NSString::from_str("public.data"),
                |content_type| content_type.identifier(),
            );
            let provider = NSFilePromiseProvider::initWithFileType_delegate(
                NSFilePromiseProvider::alloc(),
                &type_identifier,
                ProtocolObject::from_ref(&*source),
            );
            // SAFETY: the number is what `file_name` reads back.
            unsafe { provider.setUserInfo(Some(&NSNumber::new_usize(index))) };
            let dragging_item = NSDraggingItem::initWithPasteboardWriter(
                NSDraggingItem::alloc(),
                ProtocolObject::from_ref(&*provider),
            );
            let frame = NSRect::new(
                NSPoint::new(point.x - ICON_SIZE / 2.0, point.y - ICON_SIZE / 2.0),
                NSSize::new(ICON_SIZE, ICON_SIZE),
            );
            // SAFETY: the contents of a dragging frame must be an image.
            unsafe {
                dragging_item.setDraggingFrame_contents(
                    frame,
                    icon(content_type.as_deref())
                        .as_deref()
                        .map(|image| image.as_ref()),
                );
            }
            dragging_item
        })
        .collect();

    // The press that started the drag went to the web view, so AppKit gets a
    // drag event at the pointer's position to start from.
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        NSEventType::LeftMouseDragged,
        in_window,
        NSEventModifierFlags::empty(),
        NSProcessInfo::processInfo().systemUptime(),
        window.windowNumber(),
        None,
        0,
        1,
        1.0,
    )
    .ok_or("Failed to start the drag: AppKit made no drag event")?;
    view.beginDraggingSessionWithItems_event_source(
        &NSArray::from_retained_slice(&dragging_items),
        &event,
        ProtocolObject::from_ref(&*source),
    );
    SOURCE.with(|current| *current.borrow_mut() = Some(source));
    Ok(())
}
