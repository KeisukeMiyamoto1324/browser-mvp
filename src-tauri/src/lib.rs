mod browser_data;
#[cfg(target_os = "macos")]
mod native_popup;
#[cfg(target_os = "macos")]
mod native_tab_navigation;
#[cfg(target_os = "macos")]
mod native_tabs;
#[cfg(target_os = "macos")]
mod native_toolbar;
mod saved_pages;
mod start_page;
mod tabs;

use std::sync::Mutex;
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, Webview};

const TOOLBAR_HEIGHT: f64 = if cfg!(target_os = "macos") { 0.0 } else { 64.0 };

#[cfg(target_os = "macos")]
struct BrowserMenu {
    stop: tauri::menu::MenuItem<tauri::Wry>,
}

// ---------------------------------------------------------
// Accept only web URLs without changing their destination.
// ---------------------------------------------------------
fn is_web_address(url: &tauri::Url) -> bool {
    matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
}

// ---------------------------------------------------------
// Account for the macOS title bar in child WebView coordinates.
// ---------------------------------------------------------
fn content_bounds(window: &tauri::Window, viewport_height: f64) -> tauri::Result<tauri::Rect> {
    let scale = window.scale_factor()?;
    let size = window.inner_size()?.to_logical::<f64>(scale);
    let mut top = TOOLBAR_HEIGHT;
    if cfg!(target_os = "macos") && viewport_height > 0.0 {
        top += (size.height - viewport_height).max(0.0);
    }
    Ok(tauri::Rect {
        position: LogicalPosition::new(0.0, top).into(),
        size: LogicalSize::new(size.width, (size.height - top).max(1.0)).into(),
    })
}

// ---------------------------------------------------------
// Resolve an address or search query to an HTTP URL.
// ---------------------------------------------------------
fn resolve_address(input: &str) -> Result<tauri::Url, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Enter a URL or search term.".into());
    }
    if input.contains("://") {
        let url = tauri::Url::parse(input).map_err(|error| error.to_string())?;
        return is_web_address(&url)
            .then_some(url)
            .ok_or_else(|| "Only HTTP and HTTPS addresses are supported.".into());
    }
    let host = input.split('/').next().unwrap_or_default();
    if !input.chars().any(char::is_whitespace)
        && (host.contains('.') || host.starts_with("localhost") || host.starts_with('['))
    {
        let scheme = if host == "localhost" || host.starts_with("localhost:") {
            "http"
        } else {
            "https"
        };
        return tauri::Url::parse(&format!("{scheme}://{input}"))
            .map_err(|error| error.to_string());
    }
    let mut url = tauri::Url::parse("https://duckduckgo.com/").map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("q", input);
    Ok(url)
}

// ---------------------------------------------------------
// Allow browser controls only from the local toolbar.
// ---------------------------------------------------------
fn browser_content(toolbar: &Webview) -> Result<Webview, String> {
    let window = toolbar.window();
    let known = toolbar
        .state::<Mutex<tabs::Windows>>()
        .lock()
        .map_err(|error| error.to_string())?
        .owns_toolbar(toolbar.label(), window.label());
    if !known {
        return Err("Browser controls are only available to the owning toolbar.".into());
    }
    tabs::active(toolbar.app_handle(), window.label())
        .ok_or_else(|| "The browser WebView is not ready.".into())
}

#[tauri::command]
async fn resize_content(webview: Webview, viewport_height: f64) -> Result<(), String> {
    let content = browser_content(&webview)?;
    let bounds = content_bounds(&webview.window(), viewport_height).map_err(|e| e.to_string())?;
    tabs::resize(webview.app_handle(), webview.window().label(), bounds)?;
    // ---------------------------------------------------------
    // Open the first page only after the toolbar has mounted.
    // ---------------------------------------------------------
    let address = {
        let state = webview.state::<Mutex<tabs::Windows>>();
        let mut windows = state.lock().map_err(|error| error.to_string())?;
        let launch = windows
            .entries
            .get_mut(webview.window().label())
            .ok_or("The window is no longer available.")?;
        if launch.toolbar_ready {
            return Ok(());
        }
        launch.toolbar_ready = true;
        launch.pending_address.take()
    };
    let restore = {
        let state = webview.state::<Mutex<tabs::Windows>>();
        let mut windows = state.lock().map_err(|error| error.to_string())?;
        windows
            .entries
            .get_mut(webview.window().label())
            .and_then(|tabs| tabs.pending_restore.clone())
    };
    if let Some(session) = restore {
        let window = webview.window();
        return on_main(webview.app_handle(), move || {
            restore_window(&window, session)
        })
        .await;
    }
    let address =
        address.unwrap_or_else(|| tauri::Url::parse("about:blank").expect("valid blank URL"));
    if address.as_str() == "about:blank" {
        #[cfg(target_os = "macos")]
        {
            let owner = webview.window().label().to_string();
            webview
                .app_handle()
                .run_on_main_thread(move || native_toolbar::perform(&owner, "focus-address"))
                .map_err(|error| error.to_string())?;
        }
        return Ok(());
    }
    content.navigate(address).map_err(|error| error.to_string())
}

#[tauri::command]
async fn navigate(webview: Webview, address: String) -> Result<(), String> {
    let content = browser_content(&webview)?;
    content
        .navigate(resolve_address(&address)?)
        .map_err(|error| error.to_string())?;
    let state = webview.state::<Mutex<tabs::Windows>>();
    let mut windows = state.lock().map_err(|error| error.to_string())?;
    let tabs = windows
        .entries
        .get_mut(webview.window().label())
        .ok_or("The window is no longer available.")?;
    tabs.pending_focus = Some(content.label().into());
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum NavigationAction {
    Back,
    Forward,
    Reload,
    Stop,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NavigationState {
    url: String,
    title: String,
    native_toolbar: bool,
    can_go_back: bool,
    can_go_forward: bool,
    loading: bool,
    progress: f64,
}

// ---------------------------------------------------------
// Read and control WebKit on its owning main thread.
// Remote page scripts cannot forge native navigation state.
// ---------------------------------------------------------
#[tauri::command]
async fn browser_navigation(
    webview: Webview,
    action: Option<NavigationAction>,
) -> Result<NavigationState, String> {
    let content = browser_content(&webview)?;
    #[cfg(target_os = "macos")]
    {
        let (sender, mut receiver) = tauri::async_runtime::channel(1);
        let handle = content.app_handle().clone();
        let label = content.label().to_string();
        let owner = webview.window().label().to_string();
        content
            .with_webview(move |platform| {
                // ---------------------------------------------------------
                // Tauri supplies a live WKWebView on the main thread.
                // The reference never leaves this callback.
                // ---------------------------------------------------------
                if tabs::active(&handle, &owner).is_none_or(|active| active.label() != label) {
                    return;
                }
                let native = unsafe { &*platform.inner().cast::<objc2_web_kit::WKWebView>() };
                let state = unsafe {
                    match action {
                        Some(NavigationAction::Back) if native.canGoBack() => {
                            native.goBack();
                        }
                        Some(NavigationAction::Forward) if native.canGoForward() => {
                            native.goForward();
                        }
                        Some(NavigationAction::Reload) => {
                            native.reload();
                        }
                        Some(NavigationAction::Stop) => native.stopLoading(),
                        _ => {}
                    }
                    NavigationState {
                        native_toolbar: true,
                        title: native
                            .title()
                            .map(|title| title.to_string())
                            .unwrap_or_default(),
                        url: native
                            .URL()
                            .and_then(|url| url.absoluteString())
                            .map(|url| url.to_string())
                            .unwrap_or_default(),
                        can_go_back: native.canGoBack(),
                        can_go_forward: native.canGoForward(),
                        loading: native.isLoading(),
                        progress: native.estimatedProgress(),
                    }
                };
                // ---------------------------------------------------------
                // Focus after the destination is visible, never the hidden
                // blank child behind a local start or history surface.
                // ---------------------------------------------------------
                let blank = state.url.is_empty() || state.url == "about:blank";
                let windows = handle.state::<Mutex<tabs::Windows>>();
                let (local_surface, focus_content) = windows
                    .lock()
                    .ok()
                    .and_then(|mut windows| {
                        let tabs = windows.entries.get_mut(&owner)?;
                        Some((tabs.local_surface, tabs.take_focus(&label, blank)))
                    })
                    .unwrap_or((false, false));
                native.setHidden(blank || local_surface);
                if focus_content && let Some(window) = native.window() {
                    window.makeFirstResponder(Some(native));
                }
                native_toolbar::update(&state, &owner);
                native_toolbar::update_tabs(&handle, &owner);
                let _ = sender.try_send(state);
            })
            .map_err(|error| error.to_string())?;
        receiver
            .recv()
            .await
            .ok_or_else(|| "The browser WebView was closed.".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (content, action);
        Err("Native navigation is currently supported on macOS only.".into())
    }
}

#[tauri::command]
async fn history_query(
    webview: Webview,
    query: String,
) -> Result<Vec<browser_data::Visit>, String> {
    browser_content(&webview)?;
    let state = webview.state::<Mutex<browser_data::Store>>();
    let store = state.lock().map_err(|error| error.to_string())?;
    Ok(store.query(&query))
}

#[tauri::command]
async fn history_clear(webview: Webview, since: u64) -> Result<(), String> {
    browser_content(&webview)?;
    let state = webview.state::<Mutex<browser_data::Store>>();
    let mut store = state.lock().map_err(|error| error.to_string())?;
    store.clear_history(since)?;
    drop(store);
    let _ = webview.app_handle().emit("history-changed", ());
    Ok(())
}

#[tauri::command]
async fn history_reopen(webview: Webview, id: String) -> Result<(), String> {
    browser_content(&webview)?;
    let handle = webview.app_handle().clone();
    let owner = webview.window().label().to_string();
    on_main(webview.app_handle(), move || {
        restore_closed_id(&handle, &owner, &id)
    })
    .await
}

#[tauri::command]
async fn history_recent(webview: Webview) -> Result<Vec<browser_data::SessionWindow>, String> {
    browser_content(&webview)?;
    let state = webview.state::<Mutex<browser_data::Store>>();
    let store = state.lock().map_err(|error| error.to_string())?;
    Ok(store
        .data
        .closed
        .iter()
        .rev()
        .map(|session| browser_data::SessionWindow {
            id: session.id.clone(),
            tabs: session
                .tabs
                .iter()
                .map(|tab| browser_data::SessionTab {
                    url: tab.url.clone(),
                    title: tab.title.clone(),
                    interaction: Vec::new(),
                    pinned: tab.pinned,
                    pinned_url: tab.pinned_url.clone(),
                })
                .collect(),
            active: session.active,
            is_window: session.is_window,
            closed_at: session.closed_at,
            position: session.position,
        })
        .collect())
}

// ---------------------------------------------------------
// Deliver history requests after the local listener is ready.
// A remote-only popup can route the request to a local window.
// ---------------------------------------------------------
#[tauri::command]
async fn history_requested(webview: Webview) -> Result<bool, String> {
    browser_content(&webview)?;
    let state = webview.state::<Mutex<tabs::Windows>>();
    let mut windows = state.lock().map_err(|error| error.to_string())?;
    let tabs = windows
        .entries
        .get_mut(webview.window().label())
        .ok_or("Missing window.")?;
    Ok(std::mem::take(&mut tabs.pending_history))
}

#[tauri::command]
async fn history_visible(webview: Webview, visible: bool) -> Result<(), String> {
    browser_content(&webview)?;
    let app = webview.app_handle().clone();
    on_main(&app, move || {
        let content = browser_content(&webview)?;
        {
            let state = webview.state::<Mutex<tabs::Windows>>();
            let mut windows = state.lock().map_err(|error| error.to_string())?;
            let tabs = windows
                .entries
                .get_mut(webview.window().label())
                .ok_or("Missing window.")?;
            if tabs.local_surface == visible {
                return Ok(());
            }
            tabs.local_surface = visible;
            tabs.pending_focus = if visible {
                None
            } else {
                Some(tabs.active.clone())
            };
        }
        let blank = tabs::current_url(&content)?.as_str() == "about:blank";
        if visible || blank {
            content.hide().map_err(|error| error.to_string())?;
            return webview.set_focus().map_err(|error| error.to_string());
        }
        content.show().map_err(|error| error.to_string())?;
        content.set_focus().map_err(|error| error.to_string())?;
        let state = webview.state::<Mutex<tabs::Windows>>();
        let mut windows = state.lock().map_err(|error| error.to_string())?;
        if let Some(tabs) = windows.entries.get_mut(webview.window().label()) {
            tabs.pending_focus = None;
        }
        Ok(())
    })
    .await
}

// ---------------------------------------------------------
// Await main-thread work before confirming an IPC operation.
// Enqueueing a native callback does not mean it has completed.
// ---------------------------------------------------------
async fn on_main(
    app: &tauri::AppHandle,
    action: impl FnOnce() -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    app.run_on_main_thread(move || {
        let _ = sender.try_send(action());
    })
    .map_err(|error| error.to_string())?;
    receiver
        .recv()
        .await
        .ok_or("The browser closed before the action completed.")?
}

// ---------------------------------------------------------
// Restore tab order and selection after the local toolbar mounts.
// Only validated web addresses and blank tabs can be restored.
// ---------------------------------------------------------
fn restore_window(
    window: &tauri::Window,
    session: browser_data::SessionWindow,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let _main_thread = objc2_foundation::MainThreadMarker::new()
        .ok_or("Session restoration must run on the main thread.")?;
    let app = window.app_handle();
    let bounds = app
        .state::<Mutex<tabs::Windows>>()
        .lock()
        .map_err(|error| error.to_string())?
        .entries
        .get(window.label())
        .and_then(|tabs| tabs.bounds)
        .ok_or("The restored tab layout is not ready.")?;
    let mut labels = Vec::new();
    for (index, tab) in session.tabs.iter().enumerate() {
        let Ok(url) = tauri::Url::parse(&tab.url) else {
            continue;
        };
        if url.as_str() != "about:blank" && !is_web_address(&url) {
            continue;
        }
        if labels.is_empty() {
            let content = tabs::active(app, window.label()).ok_or("Missing restored tab.")?;
            tabs::restore_document(&content, tab)?;
            labels.push((index, content.label().to_string()));
        } else {
            tabs::create(
                window,
                tauri::Url::parse("about:blank").map_err(|error| error.to_string())?,
                bounds,
            )?;
            let content = tabs::active(app, window.label()).ok_or("Missing restored tab.")?;
            tabs::restore_document(&content, tab)?;
            labels.push((index, content.label().to_string()));
        }
    }
    {
        let state = app.state::<Mutex<tabs::Windows>>();
        let mut windows = state.lock().map_err(|error| error.to_string())?;
        let tabs = windows
            .entries
            .get_mut(window.label())
            .ok_or("Missing restored window.")?;
        tabs.entries.sort_by_key(|tab| !tab.pinned);
    }
    if let Some((_, label)) = labels
        .iter()
        .find(|(index, _)| *index == session.active)
        .or_else(|| labels.first())
    {
        tabs::select(app, window.label(), label)?;
    }
    app.state::<Mutex<tabs::Windows>>()
        .lock()
        .map_err(|error| error.to_string())?
        .entries
        .get_mut(window.label())
        .ok_or("The restored window was closed.")?
        .pending_restore = None;
    browser_data::checkpoint(app);
    Ok(())
}

fn restore_closed(app: &tauri::AppHandle, owner: &str) -> Result<(), String> {
    let id = app
        .state::<Mutex<browser_data::Store>>()
        .lock()
        .map_err(|error| error.to_string())?
        .data
        .closed
        .last()
        .map(|entry| entry.id.clone());
    let Some(id) = id else {
        return Ok(());
    };
    restore_closed_id(app, owner, &id)
}

fn restore_closed_id(app: &tauri::AppHandle, owner: &str, id: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let _main_thread = objc2_foundation::MainThreadMarker::new()
        .ok_or("Closed tab restoration must run on the main thread.")?;
    let session = app
        .state::<Mutex<browser_data::Store>>()
        .lock()
        .map_err(|error| error.to_string())?
        .data
        .closed
        .iter()
        .find(|session| session.id == id)
        .cloned()
        .ok_or("The recently closed item is no longer available.")?;
    if !session.is_window && session.tabs.len() == 1 && app.get_window(owner).is_some() {
        let url = tauri::Url::parse(&session.tabs[0].url).map_err(|error| error.to_string())?;
        if url.as_str() != "about:blank" && !is_web_address(&url) {
            return Err("Invalid restored address.".into());
        }
        tabs::open(
            app,
            owner,
            tauri::Url::parse("about:blank").map_err(|error| error.to_string())?,
        )?;
        let content = tabs::active(app, owner).ok_or("Missing restored tab.")?;
        tabs::restore_document(&content, &session.tabs[0])?;
        {
            let state = app.state::<Mutex<tabs::Windows>>();
            let mut windows = state.lock().map_err(|error| error.to_string())?;
            let tabs = windows
                .entries
                .get_mut(owner)
                .ok_or("Missing restored window.")?;
            if let Some(tab) = tabs.entries.pop() {
                let boundary = tabs.entries.iter().take_while(|entry| entry.pinned).count();
                let minimum = if tab.pinned { 0 } else { boundary };
                let maximum = if tab.pinned {
                    boundary
                } else {
                    tabs.entries.len()
                };
                let position = session.position.unwrap_or(maximum).clamp(minimum, maximum);
                tabs.entries.insert(position, tab);
            }
        }
        browser_data::checkpoint(app);
    } else {
        let window = create_window(app, None, None)?;
        {
            let state = app.state::<Mutex<tabs::Windows>>();
            let mut windows = state.lock().map_err(|error| error.to_string())?;
            let tabs = windows
                .entries
                .get_mut(window.label())
                .ok_or("Missing window.")?;
            tabs.toolbar_ready = true;
            tabs.pending_address = None;
            tabs.pending_restore = Some(session.clone());
        }
        restore_window(&window, session)?;
    }
    let state = app.state::<Mutex<browser_data::Store>>();
    let mut store = state.lock().map_err(|error| error.to_string())?;
    store.remove_closed(id)?;
    store.save()
}

// ---------------------------------------------------------
// Reuse the same native controls when reopening a window.
// ---------------------------------------------------------
fn initialize_window(window: &tauri::Window) -> Result<(), Box<dyn std::error::Error>> {
    window
        .state::<Mutex<tabs::Windows>>()
        .lock()
        .map_err(|error| error.to_string())?
        .entries
        .entry(window.label().to_string())
        .or_insert_with(|| tabs::Tabs {
            pending_address: Some(tauri::Url::parse("about:blank").expect("valid blank URL")),
            local_toolbar: true,
            ..Default::default()
        })
        .local_toolbar = true;
    #[cfg(target_os = "macos")]
    native_toolbar::install(window)?;
    tabs::create(window, "about:blank".parse()?, content_bounds(window, 0.0)?)?;
    Ok(())
}

// ---------------------------------------------------------
// Register each local toolbar before its first IPC request.
// Window labels are never reused during the application run.
// ---------------------------------------------------------
fn create_window(
    app: &tauri::AppHandle,
    address: Option<tauri::Url>,
    features: Option<tauri::webview::NewWindowFeatures>,
) -> Result<tauri::Window, String> {
    let address =
        address.unwrap_or_else(|| tauri::Url::parse("about:blank").expect("valid blank URL"));
    let blank = address.as_str() == "about:blank";
    let mut config = app
        .config()
        .app
        .windows
        .first()
        .ok_or("The window configuration is missing.")?
        .clone();
    let label = {
        let state = app.state::<Mutex<tabs::Windows>>();
        let mut windows = state.lock().map_err(|error| error.to_string())?;
        windows.next_id += 1;
        let label = format!("window-{}", windows.next_id);
        windows.entries.insert(
            label.clone(),
            tabs::Tabs {
                pending_address: Some(address),
                local_toolbar: true,
                ..Default::default()
            },
        );
        label
    };
    config.label = label.clone();
    if let Err(error) =
        tauri::WebviewWindowBuilder::from_config(app, &config).and_then(|mut builder| {
            // ---------------------------------------------------------
            // Apply geometry without sharing the remote opener's
            // configuration with the privileged local toolbar.
            // ---------------------------------------------------------
            if let Some(features) = features {
                if let Some(size) = features.size() {
                    builder = builder.inner_size(size.width, size.height);
                }
                if let Some(position) = features.position() {
                    builder = builder.position(position.x, position.y);
                }
            }
            builder.build()
        })
    {
        app.state::<Mutex<tabs::Windows>>()
            .lock()
            .map_err(|error| error.to_string())?
            .entries
            .remove(&label);
        return Err(error.to_string());
    }
    let window = app
        .get_window(&label)
        .ok_or("The window is no longer available.")?;
    if let Err(error) = initialize_window(&window) {
        let _ = window.close();
        return Err(error.to_string());
    }
    #[cfg(target_os = "macos")]
    if blank {
        native_toolbar::perform(&label, "focus-address");
    }
    Ok(window)
}

// ---------------------------------------------------------
// Use a separate remote-only window with the opener's config.
// The native request loads its destination after this returns.
// ---------------------------------------------------------
fn create_popup(
    app: &tauri::AppHandle,
    opener: &str,
    features: tauri::webview::NewWindowFeatures,
) -> Result<tauri::WebviewWindow, String> {
    let label = {
        let state = app.state::<Mutex<tabs::Windows>>();
        let mut windows = state.lock().map_err(|error| error.to_string())?;
        let persistent = windows
            .entries
            .get(opener)
            .ok_or("The opener window is no longer available.")?
            .persistent;
        windows.next_id += 1;
        let label = format!("popup-{}", windows.next_id);
        windows.entries.insert(
            label.clone(),
            tabs::Tabs {
                persistent,
                toolbar_ready: true,
                active: label.clone(),
                entries: vec![tabs::Tab {
                    label: label.clone(),
                    title: String::new(),
                    url: "about:blank".into(),
                    interaction: Vec::new(),
                    pinned: false,
                    pinned_url: None,
                }],
                ..Default::default()
            },
        );
        label
    };
    let build = || -> Result<tauri::WebviewWindow, String> {
        let handle = app.clone();
        let source = label.clone();
        let popup = tauri::WebviewWindowBuilder::new(
            app,
            &label,
            tauri::WebviewUrl::External(tauri::Url::parse("about:blank").expect("valid blank URL")),
        )
        .title("browser")
        .inner_size(1100.0, 760.0)
        .min_inner_size(420.0, 300.0)
        .window_features(features)
        .on_navigation(tabs::allowed_destination)
        .on_page_load(|view, payload| tabs::page_loaded(view.as_ref().clone(), payload))
        .on_document_title_changed(|view, title| tabs::update_title(view.as_ref().clone(), title))
        .on_new_window(move |url, features| tabs::requested_window(&handle, &source, url, features))
        .build()
        .map_err(|error| error.to_string())?;
        #[cfg(target_os = "macos")]
        {
            let owner = app
                .get_window(&label)
                .ok_or("The popup window is not ready.")?;
            native_toolbar::install(&owner).map_err(|error| error.to_string())?;
            native_popup::install(&popup).map_err(|error| error.to_string())?;
            native_tab_navigation::install(popup.as_ref(), &label)?;
        }
        let _ = app.emit("tabs-changed", ());
        Ok(popup)
    };
    match build() {
        Ok(popup) => Ok(popup),
        Err(error) => {
            if let Some(window) = app.get_window(&label) {
                let _ = window.close();
            }
            if let Ok(mut windows) = app.state::<Mutex<tabs::Windows>>().lock() {
                windows.entries.remove(&label);
            }
            Err(error)
        }
    }
}

fn show_history(app: &tauri::AppHandle, preferred: &str) -> Result<(), String> {
    let owner = {
        let state = app.state::<Mutex<tabs::Windows>>();
        let windows = state.lock().map_err(|error| error.to_string())?;
        if windows.owns_toolbar(preferred, preferred) {
            Some(preferred.to_string())
        } else {
            windows
                .entries
                .keys()
                .filter(|label| windows.owns_toolbar(label, label))
                .min()
                .cloned()
        }
    };
    let window = match owner.and_then(|owner| app.get_window(&owner)) {
        Some(window) => window,
        None => create_window(app, None, None)?,
    };
    {
        let state = app.state::<Mutex<tabs::Windows>>();
        let mut windows = state.lock().map_err(|error| error.to_string())?;
        let tabs = windows
            .entries
            .get_mut(window.label())
            .ok_or("Missing history window.")?;
        tabs.pending_history = true;
    }
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    window
        .emit("show-history", ())
        .map_err(|error| error.to_string())
}

fn focused_window(app: &tauri::AppHandle) -> Option<tauri::Window> {
    let label = {
        let state = app.state::<Mutex<tabs::Windows>>();
        let windows = state.lock().ok()?;
        windows
            .last_active
            .as_ref()
            .filter(|label| windows.entries.contains_key(*label))
            .cloned()
            .or_else(|| windows.entries.keys().min().cloned())?
    };
    app.get_window(&label)
}

// ---------------------------------------------------------
// Keep remote content below the fixed-height local toolbar.
// ---------------------------------------------------------
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Mutex::new(tabs::Windows::default()))
        .manage(Mutex::new(browser_data::Store::default()))
        .menu(|app| {
            let menu = tauri::menu::Menu::default(app)?;
            let history = tauri::menu::Submenu::with_items(
                app,
                "History",
                true,
                &[&tauri::menu::MenuItem::with_id(
                    app,
                    "show-history",
                    "Show All History",
                    true,
                    Some("CmdOrCtrl+Y"),
                )?],
            )?;
            menu.insert(&history, 4)?;
            #[cfg(target_os = "macos")]
            let mut stop_item = None;
            #[cfg(target_os = "macos")]
            for item in menu.items()? {
                let Some(submenu) = item.as_submenu() else {
                    continue;
                };
                match submenu.text()?.as_str() {
                    "File" => {
                        submenu.remove_at(0)?;
                        for (id, label, shortcut) in [
                            ("new-window", "New Window", "CmdOrCtrl+N"),
                            ("new-tab", "New Tab", "CmdOrCtrl+T"),
                            ("close-tab", "Close Tab", "CmdOrCtrl+W"),
                            ("reopen-tab", "Reopen Last Closed Tab", "CmdOrCtrl+Shift+T"),
                            ("focus-address", "Open Location…", "CmdOrCtrl+L"),
                        ] {
                            submenu.append(&tauri::menu::MenuItem::with_id(
                                app,
                                id,
                                label,
                                true,
                                Some(shortcut),
                            )?)?;
                        }
                    }
                    "Window" => {
                        submenu.remove_at(submenu.items()?.len() - 1)?;
                        submenu.append(&tauri::menu::MenuItem::with_id(
                            app,
                            "close-window",
                            "Close Window",
                            true,
                            Some("CmdOrCtrl+Shift+W"),
                        )?)?;
                    }
                    "View" => {
                        for (id, label, shortcut) in [
                            ("next-tab", "Show Next Tab", "Ctrl+Tab"),
                            ("previous-tab", "Show Previous Tab", "Ctrl+Shift+Tab"),
                        ] {
                            let item = tauri::menu::MenuItem::with_id(
                                app,
                                id,
                                label,
                                true,
                                Some(shortcut),
                            )?;
                            submenu.append(&item)?;
                        }
                        submenu.prepend(&tauri::menu::MenuItem::with_id(
                            app,
                            "reload-page",
                            "Reload Page",
                            true,
                            Some("CmdOrCtrl+R"),
                        )?)?;
                        let stop = tauri::menu::MenuItem::with_id(
                            app,
                            "stop-page",
                            "Stop",
                            false,
                            Some("CmdOrCtrl+."),
                        )?;
                        submenu.prepend(&stop)?;
                        stop_item = Some(stop);
                    }
                    _ => {}
                }
            }
            #[cfg(target_os = "macos")]
            {
                if let Some(stop) = stop_item {
                    app.manage(BrowserMenu { stop });
                }
            }
            Ok(menu)
        })
        .on_menu_event(|app, event| {
            let action = event.id().as_ref();
            if action == "new-window" {
                if let Err(error) = create_window(app, None, None) {
                    eprintln!("Could not create window: {error}");
                }
                return;
            }
            let window = focused_window(app);
            if action == "show-history" {
                let preferred = window
                    .as_ref()
                    .map(|window| window.label())
                    .unwrap_or_default();
                if let Err(error) = show_history(app, preferred) {
                    eprintln!("Could not show history: {error}");
                }
                return;
            }

            if action == "close-window" {
                if let Some(window) = window {
                    let _ = window.close();
                }
                return;
            }
            let owner = window
                .as_ref()
                .map(|window| window.label())
                .unwrap_or_default();
            if matches!(
                action,
                "new-tab" | "close-tab" | "reopen-tab" | "next-tab" | "previous-tab"
            ) {
                if let Err(error) = tabs::perform(app, owner, action) {
                    eprintln!("Tab action failed: {error}");
                }
                return;
            }
            #[cfg(target_os = "macos")]
            native_toolbar::perform(owner, action);
        })
        .setup(|app| {
            let path = app.path().app_data_dir()?.join("browser-session.json");
            let mut store = browser_data::Store::load(path)?;
            store.restoring = true;
            let sessions = store.data.windows.clone();
            *app.state::<Mutex<browser_data::Store>>()
                .lock()
                .map_err(|error| error.to_string())? = store;
            let window = app.get_window("main").ok_or("Main window was not found.")?;
            app.manage(Mutex::new(saved_pages::Store::load(
                app.path().app_data_dir()?.join("saved-pages.json"),
            )?));
            if let Some(session) = sessions.first() {
                app.state::<Mutex<tabs::Windows>>()
                    .lock()
                    .map_err(|error| error.to_string())?
                    .entries
                    .insert(
                        "main".into(),
                        tabs::Tabs {
                            pending_restore: Some(session.clone()),
                            ..Default::default()
                        },
                    );
            }
            initialize_window(&window)?;
            #[cfg(target_os = "macos")]
            native_toolbar::perform(window.label(), "focus-address");
            for session in sessions.into_iter().skip(1) {
                let restored = create_window(app.handle(), None, None)?;
                app.state::<Mutex<tabs::Windows>>()
                    .lock()
                    .map_err(|error| error.to_string())?
                    .entries
                    .get_mut(restored.label())
                    .ok_or("Missing window.")?
                    .pending_restore = Some(session);
            }
            app.state::<Mutex<browser_data::Store>>()
                .lock()
                .map_err(|error| error.to_string())?
                .restoring = false;
            browser_data::checkpoint(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Focused(true) = event {
                if let Ok(mut windows) = window.state::<Mutex<tabs::Windows>>().lock() {
                    windows.last_active = Some(window.label().to_string());
                }
                #[cfg(target_os = "macos")]
                native_toolbar::sync_menu(window.label());
            }
            if matches!(
                event,
                tauri::WindowEvent::Focused(false) | tauri::WindowEvent::CloseRequested { .. }
            ) {
                if let Err(error) = tabs::capture(window.app_handle(), window.label()) {
                    eprintln!("Could not capture window state: {error}");
                    return;
                }
                browser_data::checkpoint(window.app_handle());
            }
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let session =
                    window
                        .state::<Mutex<tabs::Windows>>()
                        .lock()
                        .ok()
                        .and_then(|windows| {
                            windows
                                .entries
                                .get(window.label())
                                .filter(|tabs| tabs.persistent)
                                .map(browser_data::snapshot)
                        });
                if let Some(session) = session {
                    browser_data::closed(window.app_handle(), session);
                }
            }
            if let tauri::WindowEvent::Destroyed = event {
                #[cfg(target_os = "macos")]
                {
                    native_popup::remove(window.label());
                    native_toolbar::remove(window.label());
                }
                if let Ok(mut windows) = window.state::<Mutex<tabs::Windows>>().lock() {
                    windows.entries.remove(window.label());
                    windows.closed_last_window = windows.entries.is_empty();
                    if windows.last_active.as_deref() == Some(window.label()) {
                        windows.last_active = windows.entries.keys().min().cloned();
                    }
                }
                browser_data::checkpoint(window.app_handle());
                let _ = window.app_handle().emit("tabs-changed", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            resize_content,
            saved_pages::get_saved_pages,
            saved_pages::edit_saved_pages,
            navigate,
            browser_navigation,
            history_query,
            history_clear,
            history_visible,
            history_requested,
            history_reopen,
            history_recent,
            start_page::get_start_page,
            start_page::set_start_page,
            start_page::select_suggested_tab,
        ])
        .build(tauri::generate_context!())
        .expect("error while building browser")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { code: Some(_), .. } = &event {
                let labels = app
                    .state::<Mutex<tabs::Windows>>()
                    .lock()
                    .map(|windows| windows.entries.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default();
                let captured = labels
                    .iter()
                    .try_for_each(|label| tabs::capture(app, label));
                match captured {
                    Ok(()) => browser_data::checkpoint(app),
                    Err(error) => eprintln!("Could not capture exit state: {error}"),
                }
                if let Ok(mut store) = app.state::<Mutex<browser_data::Store>>().lock() {
                    store.quitting = true;
                }
            }
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = &event
                && let Ok(mut windows) = app.state::<Mutex<tabs::Windows>>().lock()
                && std::mem::take(&mut windows.closed_last_window)
            {
                api.prevent_exit();
            }

            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = &event
            {
                if let Some(window) = focused_window(app) {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                } else if let Err(error) = create_window(app, None, None) {
                    eprintln!("Could not reopen window: {error}");
                }
            }
            // ---------------------------------------------------------
            // Handle macOS URL-open events during launch and normal use.
            // ---------------------------------------------------------
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = event {
                let Some(url) = urls.into_iter().rfind(is_web_address) else {
                    return;
                };
                let Some(window) = focused_window(app) else {
                    if let Err(error) = create_window(app, Some(url), None) {
                        eprintln!("Could not open web address: {error}");
                    }
                    return;
                };
                {
                    let state = app.state::<Mutex<tabs::Windows>>();
                    let Ok(mut windows) = state.lock() else {
                        return;
                    };
                    let Some(launch) = windows.entries.get_mut(window.label()) else {
                        return;
                    };
                    if !launch.toolbar_ready {
                        launch.pending_address = Some(url);
                        return;
                    }
                }
                let Some(content) = tabs::active(app, window.label()) else {
                    return;
                };
                if let Err(error) = content.navigate(url) {
                    eprintln!("Could not open web address: {error}");
                }
                let window = content.window();
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{is_web_address, resolve_address};

    #[test]
    fn local_navigation_focus_waits_for_the_visible_destination() {
        let mut tabs = super::tabs::Tabs {
            pending_focus: Some("content-main-1".into()),
            ..Default::default()
        };
        assert!(!tabs.take_focus("content-main-1", true));
        tabs.local_surface = true;
        assert!(!tabs.take_focus("content-main-1", false));
        tabs.local_surface = false;
        assert!(!tabs.take_focus("content-main-2", false));
        assert!(tabs.take_focus("content-main-1", false));
        assert!(!tabs.take_focus("content-main-1", false));
    }

    #[test]
    fn toolbar_access_stays_with_its_registered_window() {
        let mut windows = super::tabs::Windows::default();
        windows.entries.insert(
            "main".into(),
            super::tabs::Tabs {
                local_toolbar: true,
                ..Default::default()
            },
        );
        windows.entries.insert(
            "window-1".into(),
            super::tabs::Tabs {
                local_toolbar: true,
                ..Default::default()
            },
        );
        assert!(windows.owns_toolbar("main", "main"));
        assert!(windows.owns_toolbar("window-1", "window-1"));
        assert!(!windows.owns_toolbar("content-main-1", "main"));
        assert!(!windows.owns_toolbar("content-window-1-1", "window-1"));
        assert!(!windows.owns_toolbar("main", "window-1"));
        assert!(!windows.owns_toolbar("window-99", "window-99"));
        windows
            .entries
            .insert("popup-2".into(), super::tabs::Tabs::default());
        assert!(!windows.owns_toolbar("popup-2", "popup-2"));

        windows.entries.remove("window-1");
        assert!(!windows.owns_toolbar("window-1", "window-1"));
    }

    #[test]
    fn preserves_web_destinations_and_rejects_other_schemes() -> Result<(), String> {
        for input in [
            "http://example.com/path?value=1&next=2#section",
            "https://example.com/path?value=1&next=2#section",
        ] {
            let url = tauri::Url::parse(input).map_err(|e| e.to_string())?;
            assert!(is_web_address(&url));
            assert_eq!(resolve_address(input)?.as_str(), input);
        }
        for input in [
            "file:///etc/hosts",
            "javascript:alert(1)",
            "tauri://localhost",
            "mailto:test@example.com",
        ] {
            let url = tauri::Url::parse(input).map_err(|e| e.to_string())?;
            assert!(!is_web_address(&url));
        }
        Ok(())
    }

    #[test]
    fn popup_destinations_allow_blank_and_web_only() -> Result<(), String> {
        for address in [
            "about:blank",
            "http://127.0.0.1:19320/child",
            "https://example.com/",
        ] {
            assert!(super::tabs::allowed_destination(
                &tauri::Url::parse(address).map_err(|error| error.to_string())?
            ));
        }
        for address in [
            "about:config",
            "file:///tmp/child",
            "javascript:alert(1)",
            "tauri://localhost/",
            "mailto:test@example.invalid",
        ] {
            let url = tauri::Url::parse(address).map_err(|error| error.to_string())?;
            assert!(!super::tabs::allowed_destination(&url));
        }
        Ok(())
    }

    #[test]
    fn resolves_addresses_and_search_queries() -> Result<(), String> {
        assert_eq!(
            resolve_address(" example.com/path ")?.as_str(),
            "https://example.com/path"
        );
        assert_eq!(
            resolve_address("localhost:3000")?.as_str(),
            "http://localhost:3000/"
        );
        assert_eq!(
            resolve_address("http://example.com")?.as_str(),
            "http://example.com/"
        );
        let search = resolve_address("日本語 & Rust")?;
        assert_eq!(search.host_str(), Some("duckduckgo.com"));
        assert_eq!(
            search
                .query_pairs()
                .next()
                .map(|(_, value)| value.into_owned()),
            Some("日本語 & Rust".into())
        );
        Ok(())
    }

    #[test]
    fn rejects_empty_and_unsupported_addresses() -> Result<(), String> {
        for input in [
            " ",
            "file:///etc/hosts",
            "javascript://alert(1)",
            "https://",
        ] {
            assert!(resolve_address(input).is_err(), "{input}");
        }
        Ok(())
    }
}
