use crate::{PlatformError, PlatformErrorCode};
use objc2::{
    define_class, msg_send, rc::Retained, runtime::ProtocolObject, DefinedClass, MainThreadOnly,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSData, NSObject, NSObjectProtocol, NSString,
    NSURLBookmarkCreationOptions, NSURLBookmarkResolutionOptions, NSURL,
};
use objc2_ui_kit::{
    UIApplication, UIDocumentInteractionController, UIDocumentPickerDelegate,
    UIDocumentPickerViewController, UIViewController, UIWindowScene,
};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
};

type Done = Box<dyn FnOnce(Result<Option<PathBuf>, PlatformError>)>;
struct PickerIvars {
    done: RefCell<Option<Done>>,
    bookmarks: PathBuf,
}
struct Scope(Retained<NSURL>);
impl Drop for Scope {
    fn drop(&mut self) {
        // Every stored Scope corresponds to one successful start call.
        unsafe { self.0.stopAccessingSecurityScopedResource() };
    }
}
thread_local! {
    // UIKit's delegate property is weak. Own both objects until selection/cancel.
    static ACTIVE: RefCell<Option<(Retained<PickerDelegate>, Retained<UIDocumentPickerViewController>)>> = const { RefCell::new(None) };
    static SCOPES: RefCell<Vec<Scope>> = const { RefCell::new(Vec::new()) };
    static INTERACTION: RefCell<Option<Retained<UIDocumentInteractionController>>> = const { RefCell::new(None) };
}
fn error(message: impl Into<String>) -> PlatformError {
    PlatformError::new(
        PlatformErrorCode::ProviderUnavailable,
        "ios.documents",
        message,
    )
}
fn retain_access(url: Retained<NSURL>, bookmarks: &Path) -> Result<PathBuf, PlatformError> {
    let path = PathBuf::from(
        url.path()
            .ok_or_else(|| error("selected document has no filesystem path"))?
            .to_string(),
    );
    // App-sandbox URLs need no security scope. External URLs must retain access.
    let scoped = unsafe { url.startAccessingSecurityScopedResource() };
    let scope = scoped.then(|| Scope(url.clone()));
    let data = url
        .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
            NSURLBookmarkCreationOptions::MinimalBookmark,
            None,
            None,
        )
        .map_err(|e| error(format!("cannot persist document authorization: {e}")))?;
    std::fs::create_dir_all(bookmarks).map_err(|e| error(e.to_string()))?;
    let id = roukaku_core::Hash256::from_sha256(path.as_os_str().as_encoded_bytes()).to_string();
    let filename = bookmarks.join(format!("{}.bookmark", id.trim_start_matches("sha256:")));
    std::fs::write(filename, data.to_vec()).map_err(|e| error(e.to_string()))?;
    if let Some(scope) = scope {
        SCOPES.with(|scopes| scopes.borrow_mut().push(scope));
    }
    Ok(path)
}

/// Restore previously selected file-provider URLs before reopening a library.
/// Stale bookmarks are refreshed; an invalid/revoked authorization is an error.
pub fn restore_document_access(bookmarks: &Path) -> Result<(), PlatformError> {
    MainThreadMarker::new()
        .ok_or_else(|| error("document access must be restored on the UIKit main thread"))?;
    if !bookmarks.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(bookmarks).map_err(|e| error(e.to_string()))? {
        let path = entry.map_err(|e| error(e.to_string()))?.path();
        if path.extension().is_none_or(|e| e != "bookmark") {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| error(e.to_string()))?;
        let data = NSData::with_bytes(&bytes);
        let mut stale = objc2::runtime::Bool::NO;
        // `stale` lives through this synchronous Foundation call.
        let url = unsafe {
            NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
                &data,
                NSURLBookmarkResolutionOptions::WithoutUI,
                None,
                &mut stale,
            )
        }
        .map_err(|e| error(format!("document authorization must be renewed: {e}")))?;
        retain_access(url, bookmarks)?;
    }
    Ok(())
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = PickerIvars]
    struct PickerDelegate;
    unsafe impl NSObjectProtocol for PickerDelegate {}
    unsafe impl UIDocumentPickerDelegate for PickerDelegate {
        #[unsafe(method(documentPicker:didPickDocumentsAtURLs:))]
        fn selected(&self, _picker: &UIDocumentPickerViewController, urls: &NSArray<NSURL>) {
            let active = ACTIVE.with(|slot| slot.borrow_mut().take());
            let result = urls
                .firstObject()
                .ok_or_else(|| error("document picker returned no URL"))
                .and_then(|url| retain_access(url, &self.ivars().bookmarks))
                .map(Some);
            let done = self.ivars().done.borrow_mut().take();
            if let Some(done) = done {
                done(result);
            }
            drop(active);
        }
        #[unsafe(method(documentPickerWasCancelled:))]
        fn cancelled(&self, _picker: &UIDocumentPickerViewController) {
            let active = ACTIVE.with(|slot| slot.borrow_mut().take());
            let done = self.ivars().done.borrow_mut().take();
            if let Some(done) = done {
                done(Ok(None));
            }
            drop(active);
        }
    }
);
fn presenter(mtm: MainThreadMarker) -> Option<Retained<UIViewController>> {
    let application = UIApplication::sharedApplication(mtm);
    let scenes = application.connectedScenes().allObjects();
    for index in 0..scenes.count() {
        let scene = scenes.objectAtIndex(index);
        let Some(scene) = scene.downcast_ref::<UIWindowScene>() else {
            continue;
        };
        let windows = scene.windows();
        for index in 0..windows.count() {
            let window = windows.objectAtIndex(index);
            if !window.isKeyWindow() {
                continue;
            }
            let mut controller = window.rootViewController()?;
            while let Some(presented) = controller.presentedViewController() {
                controller = presented;
            }
            return Some(controller);
        }
    }
    None
}

/// Present the native Files picker on the main thread without blocking UIKit.
/// The callback receives None on cancellation and a persistent scoped path on success.
#[allow(deprecated)] // String UTIs also cover custom game data types on iOS 16.
pub fn pick_document(title: &str, folder: bool, bookmarks: &Path, done: Done) {
    use objc2_ui_kit::UIDocumentPickerMode;
    let Some(mtm) = MainThreadMarker::new() else {
        done(Err(error(
            "document picker must run on the UIKit main thread",
        )));
        return;
    };
    if ACTIVE.with(|slot| slot.borrow().is_some()) {
        done(Err(error("a document picker is already open")));
        return;
    }
    let Some(parent) = presenter(mtm) else {
        done(Err(error(
            "no active UIKit window can present the document picker",
        )));
        return;
    };
    let this = PickerDelegate::alloc(mtm).set_ivars(PickerIvars {
        done: RefCell::new(Some(done)),
        bookmarks: bookmarks.to_owned(),
    });
    // NSObject has no custom initializer requirements.
    let delegate: Retained<PickerDelegate> = unsafe { msg_send![super(this), init] };
    let types = NSArray::from_retained_slice(&[NSString::from_str(if folder {
        "public.folder"
    } else {
        "public.item"
    })]);
    let picker = UIDocumentPickerViewController::initWithDocumentTypes_inMode(
        UIDocumentPickerViewController::alloc(mtm),
        &types,
        UIDocumentPickerMode::Open,
    );
    picker.setTitle(Some(&NSString::from_str(title)));
    picker.setAllowsMultipleSelection(false);
    picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    ACTIVE.with(|slot| *slot.borrow_mut() = Some((delegate, picker.clone())));
    parent.presentViewController_animated_completion(&picker, true, None);
}

/// Show UIKit's native document actions rather than spawning a desktop process.
pub fn open_document(path: &Path) -> Result<(), PlatformError> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| error("document actions must run on the UIKit main thread"))?;
    let parent = presenter(mtm)
        .ok_or_else(|| error("no active UIKit window can present document actions"))?;
    let view = parent
        .view()
        .ok_or_else(|| error("UIKit presentation view is unavailable"))?;
    let path = path
        .to_str()
        .ok_or_else(|| error("document path is not UTF-8"))?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(path));
    let interaction = UIDocumentInteractionController::interactionControllerWithURL(&url, mtm);
    if !interaction.presentOptionsMenuFromRect_inView_animated(view.bounds(), &view, true) {
        return Err(error("no document actions are available for this location"));
    }
    INTERACTION.with(|slot| *slot.borrow_mut() = Some(interaction));
    Ok(())
}
