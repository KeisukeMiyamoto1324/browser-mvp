use std::cell::RefCell;
use std::collections::HashMap;

use block2::Block;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol};
use objc2_web_kit::{
    WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate, WKWebView,
};
use tauri::Manager;

struct NavigationState {
    original: Retained<AnyObject>,
    app: tauri::AppHandle,
    owner: String,
    label: String,
}

thread_local! {
    static DELEGATES: RefCell<HashMap<String, Retained<TabNavigationDelegate>>> = RefCell::new(HashMap::new());
}

// ---------------------------------------------------------
// Filter pinned links only when their target is the main
// frame. Forward all existing loading and history callbacks.
// ---------------------------------------------------------
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = NavigationState]
    struct TabNavigationDelegate;

    unsafe impl NSObjectProtocol for TabNavigationDelegate {}

    unsafe impl WKNavigationDelegate for TabNavigationDelegate {
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        fn navigation_policy(&self, view: &WKWebView, action: &WKNavigationAction, handler: &Block<dyn Fn(WKNavigationActionPolicy)>) {
            let state = self.ivars();
            let top_level = unsafe { action.targetFrame() }.is_some_and(|frame| unsafe { frame.isMainFrame() });
            let url = unsafe { action.request().URL() }
                .and_then(|url| url.absoluteString())
                .and_then(|url| tauri::Url::parse(&url.to_string()).ok());
            if top_level && url.as_ref().is_some_and(|url| crate::tabs::divert_pinned_navigation(&state.app, &state.owner, &state.label, url)) {
                handler.call((WKNavigationActionPolicy::Cancel,));
                return;
            }
            unsafe {
                let _: () = msg_send![&*state.original, webView: view, decidePolicyForNavigationAction: action, decisionHandler: handler];
            }
        }
    }

    impl TabNavigationDelegate {
        #[unsafe(method(respondsToSelector:))]
        fn responds(&self, selector: Sel) -> bool {
            selector == sel!(webView:decidePolicyForNavigationAction:decisionHandler:)
                || unsafe { msg_send![&*self.ivars().original, respondsToSelector: selector] }
        }

        #[unsafe(method(forwardingTargetForSelector:))]
        fn forward(&self, _selector: Sel) -> *const AnyObject {
            Retained::as_ptr(&self.ivars().original)
        }
    }
);

pub fn install(content: &tauri::Webview, owner: &str) -> Result<(), String> {
    let app = content.app_handle().clone();
    let owner = owner.to_string();
    let label = content.label().to_string();
    content
        .with_webview(move |platform| {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let native = unsafe { &*platform.inner().cast::<WKWebView>() };
            let Some(original) = (unsafe { native.navigationDelegate() }) else {
                return;
            };
            let state = NavigationState {
                original: unsafe { Retained::cast_unchecked(original) },
                app,
                owner,
                label: label.clone(),
            };
            let delegate = TabNavigationDelegate::alloc(mtm).set_ivars(state);
            let delegate: Retained<TabNavigationDelegate> =
                unsafe { msg_send![super(delegate), init] };
            unsafe {
                native.setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            }
            DELEGATES.with(|delegates| {
                delegates.borrow_mut().insert(label, delegate);
            });
        })
        .map_err(|error| error.to_string())
}

pub fn remove(label: &str) {
    DELEGATES.with(|delegates| {
        delegates.borrow_mut().remove(label);
    });
}

pub fn remove_window(owner: &str) {
    DELEGATES.with(|delegates| {
        delegates
            .borrow_mut()
            .retain(|_, delegate| delegate.ivars().owner != owner);
    });
}
