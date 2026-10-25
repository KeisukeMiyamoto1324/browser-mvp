use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub hide_favorites: bool,
    pub hide_recent: bool,
    pub background: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Destination {
    pub title: String,
    pub url: String,
    pub tab: Option<String>,
    pub window: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPage {
    preferences: Preferences,
    open_tabs: Vec<Destination>,
    favorites: Vec<Destination>,
    recent: Vec<Destination>,
}

// ---------------------------------------------------------
// Read only this application's live tabs on the main thread.
// Browser records from other applications are never imported.
// ---------------------------------------------------------
pub fn destinations(app: &tauri::AppHandle) -> Vec<Destination> {
    let entries = app
        .state::<Mutex<crate::tabs::Windows>>()
        .lock()
        .map(|windows| {
            windows
                .entries
                .iter()
                .filter(|(_, tabs)| tabs.persistent)
                .flat_map(|(window, tabs)| {
                    tabs.entries
                        .iter()
                        .map(|tab| (window.clone(), tab.label.clone(), tab.title.clone()))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut destinations = Vec::new();
    for (window, label, title) in entries {
        let Some(content) = app.get_webview(&label) else {
            continue;
        };
        let Ok(url) = crate::tabs::current_url(&content) else {
            continue;
        };
        if !crate::is_web_address(&url) {
            continue;
        }
        destinations.push(Destination {
            title: if title.is_empty() {
                url.host_str().unwrap_or_default().into()
            } else {
                title
            },
            url: url.to_string(),
            tab: Some(label),
            window: Some(window),
        });
    }
    destinations.sort_by(|left, right| left.title.cmp(&right.title));
    destinations
}

pub fn saved_destinations(app: &tauri::AppHandle, root_only: bool) -> Vec<Destination> {
    crate::saved_pages::snapshot(app)
        .map(|pages| {
            pages
                .bookmarks
                .into_iter()
                .filter(|bookmark| !root_only || bookmark.parent_id.is_none())
                .filter_map(|bookmark| {
                    bookmark.url.map(|url| Destination {
                        title: bookmark.title,
                        url,
                        tab: None,
                        window: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------
// Show each recent destination once, keeping its newest visit.
// History and saved-page storage remain shared with their views.
// ---------------------------------------------------------
pub fn recent(app: &tauri::AppHandle, query: &str) -> Vec<Destination> {
    let visits = app
        .state::<Mutex<crate::browser_data::Store>>()
        .lock()
        .map(|store| store.query(query))
        .unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    visits
        .into_iter()
        .filter(|visit| seen.insert(visit.url.clone()))
        .map(|visit| Destination {
            title: if visit.title.is_empty() {
                visit.url.clone()
            } else {
                visit.title
            },
            url: visit.url,
            tab: None,
            window: None,
        })
        .collect()
}

#[tauri::command]
pub async fn get_start_page(webview: tauri::Webview) -> Result<StartPage, String> {
    crate::browser_content(&webview)?;
    let app = webview.app_handle().clone();
    let preferences = app
        .path()
        .app_data_dir()
        .ok()
        .and_then(|directory| std::fs::read(directory.join("start-page.json")).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let (sender, receiver) = tauri::async_runtime::channel(1);
    let handle = app.clone();
    app.clone()
        .run_on_main_thread(move || {
            let _ = sender.try_send(destinations(&handle));
        })
        .map_err(|error| error.to_string())?;
    let mut receiver = receiver;
    let favorites = saved_destinations(&app, true);
    Ok(StartPage {
        preferences,
        favorites,
        recent: recent(&app, "").into_iter().take(12).collect(),
        open_tabs: receiver.recv().await.unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn set_start_page(
    webview: tauri::Webview,
    preferences: Preferences,
) -> Result<(), String> {
    crate::browser_content(&webview)?;
    let directory = webview
        .app_handle()
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(&preferences).map_err(|error| error.to_string())?;
    std::fs::write(directory.join("start-page.json"), bytes).map_err(|error| error.to_string())?;
    let _ = webview.app_handle().emit("start-page-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn select_suggested_tab(
    webview: tauri::Webview,
    window: String,
    tab: String,
) -> Result<(), String> {
    crate::browser_content(&webview)?;
    let app = webview.app_handle().clone();
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    app.clone()
        .run_on_main_thread(move || {
            let result = crate::tabs::select(&app, &window, &tab).and_then(|()| {
                app.get_window(&window)
                    .ok_or_else(|| "The window is no longer available.".to_string())?
                    .set_focus()
                    .map_err(|error| error.to_string())
            });
            let _ = sender.try_send(result);
        })
        .map_err(|error| error.to_string())?;
    receiver
        .recv()
        .await
        .ok_or("The window is no longer available.")?
}
