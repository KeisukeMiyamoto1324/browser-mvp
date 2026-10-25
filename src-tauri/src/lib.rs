#[cfg(target_os = "macos")]
mod native_tabs;
#[cfg(target_os = "macos")]
mod native_toolbar;
mod saved_pages;
mod tabs;

use std::sync::Mutex;
use tauri::{LogicalPosition, LogicalSize, Manager, Webview};

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
    browser_content(&webview)?
        .navigate(resolve_address(&address)?)
        .map_err(|error| error.to_string())
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
            ..Default::default()
        });
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
        .menu(|app| {
            let menu = tauri::menu::Menu::default(app)?;
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
                            submenu.append(&tauri::menu::MenuItem::with_id(
                                app,
                                id,
                                label,
                                true,
                                Some(shortcut),
                            )?)?;
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
                        app.manage(BrowserMenu { stop });
                    }
                    _ => {}
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
            let window = app.get_window("main").ok_or("Main window was not found.")?;
            app.manage(Mutex::new(saved_pages::Store::load(
                app.path().app_data_dir()?.join("saved-pages.json"),
            )?));
            initialize_window(&window)?;
            #[cfg(target_os = "macos")]
            native_toolbar::perform(window.label(), "focus-address");
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
            if let tauri::WindowEvent::Destroyed = event {
                #[cfg(target_os = "macos")]
                native_toolbar::remove(window.label());
                if let Ok(mut windows) = window.state::<Mutex<tabs::Windows>>().lock() {
                    windows.entries.remove(window.label());
                    windows.closed_last_window = windows.entries.is_empty();
                    if windows.last_active.as_deref() == Some(window.label()) {
                        windows.last_active = windows.entries.keys().min().cloned();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            resize_content,
            saved_pages::get_saved_pages,
            saved_pages::edit_saved_pages,
            navigate,
            browser_navigation
        ])
        .build(tauri::generate_context!())
        .expect("error while building browser")
        .run(|app, event| {
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
    fn toolbar_access_stays_with_its_registered_window() {
        let mut windows = super::tabs::Windows::default();
        windows
            .entries
            .insert("main".into(), super::tabs::Tabs::default());
        windows
            .entries
            .insert("window-1".into(), super::tabs::Tabs::default());
        assert!(windows.owns_toolbar("main", "main"));
        assert!(windows.owns_toolbar("window-1", "window-1"));
        assert!(!windows.owns_toolbar("content-main-1", "main"));
        assert!(!windows.owns_toolbar("content-window-1-1", "window-1"));
        assert!(!windows.owns_toolbar("main", "window-1"));
        assert!(!windows.owns_toolbar("window-99", "window-99"));
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
