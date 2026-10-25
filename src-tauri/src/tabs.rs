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
    next_id: u64,
    closed: Vec<tauri::Url>,
    bounds: Option<tauri::Rect>,
}

// ---------------------------------------------------------
// Keep labels unique after closing and reopening tabs.
// Each WebView retains its own document and navigation stack.
// ---------------------------------------------------------
pub fn active(app: &AppHandle) -> Option<Webview> {
    let label = app.state::<Mutex<Tabs>>().lock().ok()?.active.clone();
    app.get_webview(&label)
}

pub fn resize(app: &AppHandle, bounds: tauri::Rect) -> Result<(), String> {
    let entries = {
        let state = app.state::<Mutex<Tabs>>();
        let mut tabs = state.lock().map_err(|error| error.to_string())?;
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

pub fn create(window: &Window, url: tauri::Url, bounds: tauri::Rect) -> Result<(), String> {
    let app = window.app_handle();
    let label = {
        let state = app.state::<Mutex<Tabs>>();
        let mut tabs = state.lock().map_err(|error| error.to_string())?;
        tabs.next_id += 1;
        format!("content-{}", tabs.next_id)
    };
    let handle = app.clone();
    let builder = tauri::webview::WebviewBuilder::new(&label, WebviewUrl::External(url))
        .focused(false)
        .on_navigation(|url| url.as_str() == "about:blank" || crate::is_web_address(url))
        .on_document_title_changed(|view, title| {
            let state = view.state::<Mutex<Tabs>>();
            if let Ok(mut tabs) = state.lock()
                && let Some(tab) = tabs
                    .entries
                    .iter_mut()
                    .find(|tab| tab.label == view.label())
            {
                tab.title = title;
            }
        })
        .on_new_window(move |url, _features| {
            if crate::is_web_address(&url) {
                let queued = handle.clone();
                let _ = handle.run_on_main_thread(move || {
                    if let Err(error) = open(&queued, url) {
                        eprintln!("Could not open tab: {error}");
                    }
                });
            }
            tauri::webview::NewWindowResponse::Deny
        });
    let content = window
        .add_child(builder, bounds.position, bounds.size)
        .map_err(|error| error.to_string())?;
    content.hide().map_err(|error| error.to_string())?;
    {
        let state = app.state::<Mutex<Tabs>>();
        let mut tabs = state.lock().map_err(|error| error.to_string())?;
        tabs.bounds = Some(bounds);
        tabs.entries.push(Tab {
            label: label.clone(),
            title: String::new(),
        });
    }
    select(app, &label)
}

pub fn select(app: &AppHandle, label: &str) -> Result<(), String> {
    let content = app
        .get_webview(label)
        .ok_or("The tab is no longer available.")?;
    let previous = {
        let state = app.state::<Mutex<Tabs>>();
        let tabs = state.lock().map_err(|error| error.to_string())?;
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
    app.state::<Mutex<Tabs>>()
        .lock()
        .map_err(|error| error.to_string())?
        .active = label.into();
    #[cfg(target_os = "macos")]
    crate::native_toolbar::tab_changed(app);
    content.set_focus().map_err(|error| error.to_string())
}

pub fn open(app: &AppHandle, url: tauri::Url) -> Result<(), String> {
    if app.get_window("main").is_none() {
        {
            let state = app.state::<Mutex<crate::BrowserLaunch>>();
            let mut launch = state.lock().map_err(|error| error.to_string())?;
            launch.toolbar_ready = false;
            launch.pending_address = Some(url);
        }
        let config = app
            .config()
            .app
            .windows
            .first()
            .ok_or("The window configuration is missing.")?;
        tauri::WebviewWindowBuilder::from_config(app, config)
            .map_err(|error| error.to_string())?
            .build()
            .map_err(|error| error.to_string())?;
        let window = app
            .get_window("main")
            .ok_or("The window is no longer available.")?;
        crate::initialize_window(&window).map_err(|error| error.to_string())?;
        return Ok(());
    }
    let window = app
        .get_window("main")
        .ok_or("The window is no longer available.")?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    let bounds = app
        .state::<Mutex<Tabs>>()
        .lock()
        .map_err(|error| error.to_string())?
        .bounds
        .ok_or("The tab layout is not ready.")?;
    create(&window, url, bounds)
}

// ---------------------------------------------------------
// Restore closed destinations without reusing WebView labels.
// Window and document restoration will extend this lifecycle.
// ---------------------------------------------------------
pub fn perform(app: &AppHandle, action: &str) -> Result<(), String> {
    if matches!(action, "next-tab" | "previous-tab") {
        let label = {
            let state = app.state::<Mutex<Tabs>>();
            let tabs = state.lock().map_err(|error| error.to_string())?;
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
        return select(app, &label);
    }

    if action == "new-tab" {
        open(
            app,
            tauri::Url::parse("about:blank").map_err(|error| error.to_string())?,
        )?;
        #[cfg(target_os = "macos")]
        crate::native_toolbar::perform("focus-address");
        return Ok(());
    }
    if action == "reopen-tab" {
        let url = app
            .state::<Mutex<Tabs>>()
            .lock()
            .map_err(|error| error.to_string())?
            .closed
            .last()
            .cloned();
        if let Some(url) = url {
            open(app, url)?;
            app.state::<Mutex<Tabs>>()
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
    let content = active(app).ok_or("The active tab is no longer available.")?;
    close_tab(app, content.label())
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
        let url = receiver
            .try_recv()
            .map_err(|_| "Tab actions must run on the main thread.")?;
        Ok(url)
    }
    #[cfg(not(target_os = "macos"))]
    content.url().map_err(|error| error.to_string())
}

pub fn close_tab(app: &AppHandle, label: &str) -> Result<(), String> {
    let content = app
        .get_webview(label)
        .ok_or("The tab is no longer available.")?;
    let url = current_url(&content)?;
    let (index, next, was_active) = {
        let state = app.state::<Mutex<Tabs>>();
        let tabs = state.lock().map_err(|error| error.to_string())?;
        let index = tabs
            .entries
            .iter()
            .position(|tab| tab.label == content.label())
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
        select(app, next)?;
    }
    content.close().map_err(|error| error.to_string())?;
    {
        let state = app.state::<Mutex<Tabs>>();
        let mut tabs = state.lock().map_err(|error| error.to_string())?;
        tabs.entries.remove(index);
        tabs.closed.push(url);
        if tabs.closed.len() > 20 {
            tabs.closed.remove(0);
        }
        if next.is_none() {
            tabs.active.clear();
        }
    }
    if next.is_none() {
        content
            .window()
            .close()
            .map_err(|error| error.to_string())?;
    } else {
        #[cfg(target_os = "macos")]
        crate::native_toolbar::update_tabs(app);
    }
    Ok(())
}
