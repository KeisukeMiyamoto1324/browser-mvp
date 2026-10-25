use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, Webview, WebviewUrl, Window};

#[derive(Clone, PartialEq)]
pub struct Tab {
    pub label: String,
    pub title: String,
}

#[derive(Default)]
pub struct Tabs {
    pub entries: Vec<Tab>,
    pub active: String,
    pub toolbar_ready: bool,
    pub pending_address: Option<tauri::Url>,
    pub(crate) next_id: u64,
    pub(crate) bounds: Option<tauri::Rect>,
}

#[derive(Default)]
pub struct Windows {
    pub entries: HashMap<String, Tabs>,
    pub last_active: Option<String>,
    pub next_id: u64,
    pub closed_last_window: bool,
    closed: Vec<tauri::Url>,
}

impl Windows {
    pub fn owns_toolbar(&self, label: &str, window: &str) -> bool {
        label == window && self.entries.contains_key(window)
    }
}

// ---------------------------------------------------------
// Resolve content within its owning window, never by focus.
// Native menu routing supplies the foreground window label.
// ---------------------------------------------------------
pub fn active(app: &AppHandle, window: &str) -> Option<Webview> {
    let label = app
        .state::<Mutex<Windows>>()
        .lock()
        .ok()?
        .entries
        .get(window)?
        .active
        .clone();
    app.get_webview(&label)
}

pub fn resize(app: &AppHandle, window: &str, bounds: tauri::Rect) -> Result<(), String> {
    let entries = {
        let state = app.state::<Mutex<Windows>>();
        let mut windows = state.lock().map_err(|error| error.to_string())?;
        let tabs = windows
            .entries
            .get_mut(window)
            .ok_or("The window is no longer available.")?;
        tabs.bounds = Some(bounds);
        tabs.entries.clone()
    };
    for tab in entries {
        if let Some(content) = app.get_webview(&tab.label) {
            content
                .set_bounds(bounds)
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------
// Keep WebView labels unique across windows and closed tabs.
// Each view retains its own document and navigation stack.
// ---------------------------------------------------------
pub fn create(window: &Window, url: tauri::Url, bounds: tauri::Rect) -> Result<(), String> {
    let app = window.app_handle();
    let label = {
        let state = app.state::<Mutex<Windows>>();
        let mut windows = state.lock().map_err(|error| error.to_string())?;
        let tabs = windows
            .entries
            .get_mut(window.label())
            .ok_or("The window is no longer available.")?;
        tabs.next_id += 1;
        format!("content-{}-{}", window.label(), tabs.next_id)
    };
    let handle = app.clone();
    let owner = window.label().to_string();
    let builder = tauri::webview::WebviewBuilder::new(&label, WebviewUrl::External(url))
        .focused(false)
        .on_navigation(|url| url.as_str() == "about:blank" || crate::is_web_address(url))
        .on_document_title_changed(|view, title| {
            let state = view.state::<Mutex<Windows>>();
            if let Ok(mut windows) = state.lock()
                && let Some(tabs) = windows.entries.get_mut(view.window().label())
                && let Some(tab) = tabs
                    .entries
                    .iter_mut()
                    .find(|tab| tab.label == view.label())
            {
                tab.title = title;
            }
        })
        .on_new_window(move |url, features| {
            if crate::is_web_address(&url) {
                let result = if features.size().is_some() || features.position().is_some() {
                    crate::create_window(&handle, Some(url), Some(features)).map(|_| ())
                } else {
                    open(&handle, &owner, url).map(|_| ())
                };
                if let Err(error) = result {
                    eprintln!("Could not open web address: {error}");
                }
            }
            tauri::webview::NewWindowResponse::Deny
        });
    let content = window
        .add_child(builder, bounds.position, bounds.size)
        .map_err(|error| error.to_string())?;
    // ---------------------------------------------------------
    // Resize with AppKit during native window animations.
    // Child WebViews otherwise keep fixed dimensions in Wry.
    // ---------------------------------------------------------
    #[cfg(target_os = "macos")]
    content
        .with_webview(|platform| {
            let native = unsafe { &*platform.inner().cast::<objc2_web_kit::WKWebView>() };
            native.setAutoresizingMask(
                objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
                    | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
            );
        })
        .map_err(|error| error.to_string())?;
    content.hide().map_err(|error| error.to_string())?;
    {
        let state = app.state::<Mutex<Windows>>();
        let mut windows = state.lock().map_err(|error| error.to_string())?;
        let tabs = windows
            .entries
            .get_mut(window.label())
            .ok_or("The window is no longer available.")?;
        tabs.bounds = Some(bounds);
        tabs.entries.push(Tab {
            label: label.clone(),
            title: String::new(),
        });
    }
    select(app, window.label(), &label)
}

pub fn select(app: &AppHandle, window: &str, label: &str) -> Result<(), String> {
    let content = app
        .get_webview(label)
        .ok_or("The tab is no longer available.")?;
    let previous = {
        let state = app.state::<Mutex<Windows>>();
        let windows = state.lock().map_err(|error| error.to_string())?;
        let tabs = windows
            .entries
            .get(window)
            .ok_or("The window is no longer available.")?;
        if !tabs.entries.iter().any(|tab| tab.label == label) {
            return Err("Unknown tab.".into());
        }
        tabs.active.clone()
    };
    content.show().map_err(|error| error.to_string())?;
    if previous != label
        && let Some(old) = app.get_webview(&previous)
        && let Err(error) = old.hide()
    {
        let _ = content.hide();
        return Err(error.to_string());
    }
    {
        let state = app.state::<Mutex<Windows>>();
        let mut windows = state.lock().map_err(|error| error.to_string())?;
        windows
            .entries
            .get_mut(window)
            .ok_or("The window is no longer available.")?
            .active = label.into();
    }
    #[cfg(target_os = "macos")]
    crate::native_toolbar::tab_changed(app, window);
    content.set_focus().map_err(|error| error.to_string())
}

pub fn open(app: &AppHandle, owner: &str, url: tauri::Url) -> Result<Window, String> {
    let Some(window) = app.get_window(owner) else {
        return crate::create_window(app, Some(url), None);
    };
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    let bounds = if let Some(content) = active(app, owner) {
        content.bounds().map_err(|error| error.to_string())?
    } else {
        app.state::<Mutex<Windows>>()
            .lock()
            .map_err(|error| error.to_string())?
            .entries
            .get(owner)
            .and_then(|tabs| tabs.bounds)
            .ok_or("The tab layout is not ready.")?
    };
    create(&window, url, bounds)?;
    Ok(window)
}

// ---------------------------------------------------------
// Keep closed destinations available after a window closes.
// Full closed-window and document restoration is separate.
// ---------------------------------------------------------
pub fn perform(app: &AppHandle, window: &str, action: &str) -> Result<(), String> {
    if matches!(action, "next-tab" | "previous-tab") {
        let label = {
            let state = app.state::<Mutex<Windows>>();
            let windows = state.lock().map_err(|error| error.to_string())?;
            let tabs = windows
                .entries
                .get(window)
                .ok_or("The window is no longer available.")?;
            let count = tabs.entries.len();
            let index = tabs
                .entries
                .iter()
                .position(|tab| tab.label == tabs.active)
                .ok_or("No active tab.")?;
            let next = if action == "next-tab" {
                (index + 1) % count
            } else {
                (index + count - 1) % count
            };
            tabs.entries[next].label.clone()
        };
        return select(app, window, &label);
    }
    if action == "new-tab" {
        let window = open(
            app,
            window,
            tauri::Url::parse("about:blank").map_err(|error| error.to_string())?,
        )?;
        #[cfg(target_os = "macos")]
        crate::native_toolbar::perform(window.label(), "focus-address");
        return Ok(());
    }
    if action == "reopen-tab" {
        let url = app
            .state::<Mutex<Windows>>()
            .lock()
            .map_err(|error| error.to_string())?
            .closed
            .last()
            .cloned();
        if let Some(url) = url {
            open(app, window, url)?;
            app.state::<Mutex<Windows>>()
                .lock()
                .map_err(|error| error.to_string())?
                .closed
                .pop();
        }
        return Ok(());
    }
    if action != "close-tab" {
        return Ok(());
    }
    let content = active(app, window).ok_or("The active tab is no longer available.")?;
    close_tab(app, window, content.label())
}

pub fn current_url(content: &Webview) -> Result<tauri::Url, String> {
    #[cfg(target_os = "macos")]
    {
        // ---------------------------------------------------------
        // Read only inside the callback: the dispatcher holds a lock.
        // Perform WebView mutations after it returns on main.
        // ---------------------------------------------------------
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        content
            .with_webview(move |platform| {
                let native = unsafe { &*platform.inner().cast::<objc2_web_kit::WKWebView>() };
                let url = unsafe { native.URL() }
                    .and_then(|url| url.absoluteString())
                    .and_then(|url| tauri::Url::parse(&url.to_string()).ok())
                    .unwrap_or_else(|| tauri::Url::parse("about:blank").expect("valid blank URL"));
                let _ = sender.try_send(url);
            })
            .map_err(|error| error.to_string())?;
        receiver
            .try_recv()
            .map_err(|_| "Tab actions must run on the main thread.".into())
    }
    #[cfg(not(target_os = "macos"))]
    content.url().map_err(|error| error.to_string())
}

pub fn close_tab(app: &AppHandle, window: &str, label: &str) -> Result<(), String> {
    let content = app
        .get_webview(label)
        .ok_or("The tab is no longer available.")?;
    let url = current_url(&content)?;
    let (index, next, was_active) = {
        let state = app.state::<Mutex<Windows>>();
        let windows = state.lock().map_err(|error| error.to_string())?;
        let tabs = windows
            .entries
            .get(window)
            .ok_or("The window is no longer available.")?;
        let index = tabs
            .entries
            .iter()
            .position(|tab| tab.label == label)
            .ok_or("Unknown tab.")?;
        let next = tabs
            .entries
            .get(index + 1)
            .or_else(|| {
                index
                    .checked_sub(1)
                    .and_then(|index| tabs.entries.get(index))
            })
            .map(|tab| tab.label.clone());
        (
            index,
            if tabs.active == label {
                next
            } else {
                Some(tabs.active.clone())
            },
            tabs.active == label,
        )
    };
    if was_active && let Some(next) = &next {
        select(app, window, next)?;
    }
    content.close().map_err(|error| error.to_string())?;
    {
        let state = app.state::<Mutex<Windows>>();
        let mut windows = state.lock().map_err(|error| error.to_string())?;
        let tabs = windows
            .entries
            .get_mut(window)
            .ok_or("The window is no longer available.")?;
        tabs.entries.remove(index);
        if next.is_none() {
            tabs.active.clear();
        }
        windows.closed.push(url);
        if windows.closed.len() > 20 {
            windows.closed.remove(0);
        }
    }
    if next.is_none() {
        content
            .window()
            .close()
            .map_err(|error| error.to_string())?;
    } else {
        #[cfg(target_os = "macos")]
        crate::native_toolbar::update_tabs(app, window);
    }
    Ok(())
}
