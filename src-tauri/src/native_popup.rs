use std::cell::RefCell;
use std::collections::HashMap;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_foundation::{
    MainThreadMarker, NSObject, NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes, NSTimer,
};
use objc2_web_kit::{WKUIDelegate, WKWebView};
use tauri::Manager;

struct PopupState {
    original: Retained<ProtocolObject<dyn WKUIDelegate>>,
    handle: tauri::AppHandle,
    owner: String,
    label: String,
}

struct PopupControls {
    _delegate: Retained<PopupDelegate>,
    timer: Retained<NSTimer>,
}

thread_local! {
    static POPUPS: RefCell<HashMap<String, PopupControls>> = RefCell::new(HashMap::new());
}

// ---------------------------------------------------------
// Forward optional dialogs and new-window callbacks to Wry.
// Add native close handling without injecting page scripts.
// ---------------------------------------------------------
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = PopupState]
    struct PopupDelegate;

    unsafe impl NSObjectProtocol for PopupDelegate {
        #[unsafe(method(respondsToSelector:))]
        fn responds(&self, selector: Sel) -> bool {
            (unsafe { msg_send![super(self), respondsToSelector: selector] })
                || self.ivars().original.respondsToSelector(selector)
        }
    }

    unsafe impl WKUIDelegate for PopupDelegate {
        #[unsafe(method(webViewDidClose:))]
        unsafe fn closed(&self, _view: &WKWebView) {
            let state = self.ivars();
            let handle = state.handle.clone();
            let owner = state.owner.clone();
            let label = state.label.clone();
            // ---------------------------------------------------------
            // Queue after WebKit releases its delegate callback.
            // Close the requested tab even if another tab is active.
            // ---------------------------------------------------------
            tauri::async_runtime::spawn(async move {
                let app = handle.clone();
                let _ = handle.run_on_main_thread(move || {
                    if let Err(error) = crate::tabs::close_tab(&app, &owner, &label) {
                        eprintln!("Could not close requested child: {error}");
                    }
                });
            });
        }
    }

    impl PopupDelegate {
        #[unsafe(method_id(forwardingTargetForSelector:))]
        fn forward(&self, _selector: Sel) -> Option<Retained<AnyObject>> {
            Some(unsafe { Retained::cast_unchecked(self.ivars().original.clone()) })
        }

        #[unsafe(method(poll:))]
        fn poll(&self, _timer: &NSTimer) {
            let state = self.ivars();
            let Some(content) = crate::tabs::active(&state.handle, &state.owner) else { return; };
            let owner = state.owner.clone();
            let handle = state.handle.clone();
            let _ = content.with_webview(move |platform| {
                let native = unsafe { &*platform.inner().cast::<WKWebView>() };
                let navigation = unsafe {
                    crate::NavigationState {
                        native_toolbar: true,
                        title: native.title().map(|title| title.to_string()).unwrap_or_default(),
                        url: native.URL().and_then(|url| url.absoluteString()).map(|url| url.to_string()).unwrap_or_default(),
                        can_go_back: native.canGoBack(),
                        can_go_forward: native.canGoForward(),
                        loading: native.isLoading(),
                        progress: native.estimatedProgress(),
                    }
                };
                crate::native_toolbar::update(&navigation, &owner);
                crate::native_toolbar::update_tabs(&handle, &owner);
            });
        }
    }
);

pub fn install(popup: &tauri::WebviewWindow) -> Result<(), Box<dyn std::error::Error>> {
    MainThreadMarker::new().ok_or("Popup setup must run on the main thread.")?;
    let handle = popup.app_handle().clone();
    let owner = popup.label().to_string();
    popup.with_webview(move |platform| {
        let mtm = MainThreadMarker::new().expect("WebKit callback runs on the main thread");
        let native = unsafe { &*platform.inner().cast::<WKWebView>() };
        let Some(original) = (unsafe { native.UIDelegate() }) else {
            return;
        };
        let delegate = PopupDelegate::alloc(mtm).set_ivars(PopupState {
            original,
            handle,
            owner: owner.clone(),
            label: owner.clone(),
        });
        let delegate: Retained<PopupDelegate> = unsafe { msg_send![super(delegate), init] };
        unsafe {
            native.setUIDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            let timer = NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
                0.1,
                &delegate,
                sel!(poll:),
                None,
                true,
            );
            NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes);
            POPUPS.with(|slot| {
                slot.borrow_mut().insert(
                    owner,
                    PopupControls {
                        _delegate: delegate,
                        timer,
                    },
                );
            });
        }
    })?;
    Ok(())
}

pub fn remove(owner: &str) {
    POPUPS.with(|slot| {
        if let Some(controls) = slot.borrow_mut().remove(owner) {
            controls.timer.invalidate();
        }
    });
}
