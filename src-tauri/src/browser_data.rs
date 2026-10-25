use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Visit {
    pub url: String,
    pub title: String,
    pub visited_at: u64,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct SessionTab {
    pub url: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interaction: Vec<u8>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct SessionWindow {
    #[serde(default)]
    pub id: String,
    pub tabs: Vec<SessionTab>,
    pub active: usize,
    #[serde(default)]
    pub is_window: bool,
    #[serde(default)]
    pub closed_at: u64,
    #[serde(default)]
    pub position: Option<usize>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Data {
    pub visits: Vec<Visit>,
    pub windows: Vec<SessionWindow>,
    pub closed: Vec<SessionWindow>,
    pub next_closed_id: u64,
}

#[derive(Default)]
pub struct Store {
    pub data: Data,
    pub path: PathBuf,
    pub quitting: bool,
    pub restoring: bool,
}

impl Store {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        let mut data: Data = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| error.to_string())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Data::default(),
            Err(error) => return Err(error.to_string()),
        };
        let existing: std::collections::HashSet<String> =
            data.closed.iter().map(|entry| entry.id.clone()).collect();
        let mut ids = std::collections::HashSet::new();
        for session in &mut data.closed {
            if session.id.is_empty() || !ids.insert(session.id.clone()) {
                loop {
                    data.next_closed_id = data
                        .next_closed_id
                        .checked_add(1)
                        .ok_or("Recently closed IDs are exhausted.")?;
                    let id = format!("closed-{}", data.next_closed_id);
                    if ids.insert(id.clone()) && !existing.contains(&id) {
                        session.id = id;
                        break;
                    }
                }
            }
        }
        Ok(Self {
            data,
            path,
            quitting: false,
            restoring: false,
        })
    }

    // ---------------------------------------------------------
    // Replace a fully synced file in the same directory.
    // A failed write leaves the previous session intact.
    // ---------------------------------------------------------
    pub fn save(&self) -> Result<(), String> {
        let parent = self.path.parent().ok_or("Invalid storage path.")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = self.path.with_extension("json.tmp");
        let mut options = File::options();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(&serde_json::to_vec(&self.data).map_err(|error| error.to_string())?)
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::rename(&temporary, &self.path))
            .map_err(|error| error.to_string())?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| error.to_string())
    }

    pub fn replace_windows(&mut self, windows: Vec<SessionWindow>) -> Result<(), String> {
        if self.quitting || self.restoring {
            return Ok(());
        }
        self.data.windows = windows;
        self.save()
    }

    pub fn remember_closed(&mut self, mut session: SessionWindow) -> Result<(), String> {
        loop {
            self.data.next_closed_id = self
                .data
                .next_closed_id
                .checked_add(1)
                .ok_or("Recently closed IDs are exhausted.")?;
            session.id = format!("closed-{}", self.data.next_closed_id);
            if !self.data.closed.iter().any(|entry| entry.id == session.id) {
                break;
            }
        }
        session.closed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.data.closed.push(session);
        if self.data.closed.len() > 20 {
            self.data.closed.remove(0);
        }
        Ok(())
    }

    pub fn remove_closed(&mut self, id: &str) -> Result<(), String> {
        let position = self
            .data
            .closed
            .iter()
            .position(|entry| entry.id == id)
            .ok_or("The recently closed item is no longer available.")?;
        self.data.closed.remove(position);
        Ok(())
    }

    pub fn record(&mut self, url: &str, title: &str) {
        let Ok(address) = tauri::Url::parse(url) else {
            return;
        };
        if !crate::is_web_address(&address) {
            return;
        }
        self.data.visits.push(Visit {
            url: url.into(),
            title: title.into(),
            visited_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        });
        if self.data.visits.len() > 50_000 {
            self.data.visits.remove(0);
        }
    }

    pub fn query(&self, query: &str) -> Vec<Visit> {
        let query = query.to_lowercase();
        self.data
            .visits
            .iter()
            .rev()
            .filter(|visit| {
                visit.url.to_lowercase().contains(&query)
                    || visit.title.to_lowercase().contains(&query)
            })
            .take(1000)
            .cloned()
            .collect()
    }
}

pub fn snapshot(tabs: &crate::tabs::Tabs) -> SessionWindow {
    if let Some(pending) = &tabs.pending_restore {
        return pending.clone();
    }
    SessionWindow {
        id: String::new(),
        is_window: true,
        closed_at: 0,
        position: None,
        tabs: tabs
            .entries
            .iter()
            .map(|tab| SessionTab {
                url: tab.url.clone(),
                title: tab.title.clone(),
                interaction: tab.interaction.clone(),
            })
            .collect(),
        active: tabs
            .entries
            .iter()
            .position(|tab| tab.label == tabs.active)
            .unwrap_or(0),
    }
}

fn sessions(windows: &crate::tabs::Windows) -> Vec<SessionWindow> {
    let mut entries: Vec<_> = windows.entries.iter().collect();
    entries.sort_by_key(|(label, _)| *label);
    entries
        .into_iter()
        .filter(|(_, tabs)| tabs.persistent)
        .map(|(_, tabs)| snapshot(tabs))
        .filter(|window| !window.tabs.is_empty())
        .collect()
}

pub fn checkpoint(app: &AppHandle) {
    let windows = app.state::<Mutex<crate::tabs::Windows>>();
    let Ok(windows) = windows.lock() else { return };
    let sessions = sessions(&windows);
    let state = app.state::<Mutex<Store>>();
    let Ok(mut store) = state.lock() else { return };
    if let Err(error) = store.replace_windows(sessions) {
        eprintln!("Could not save browser data: {error}");
    }
}

pub fn closed(app: &AppHandle, session: SessionWindow) {
    if session.tabs.is_empty() {
        return;
    }
    let state = app.state::<Mutex<Store>>();
    let Ok(mut store) = state.lock() else { return };
    if store.quitting {
        return;
    }
    if let Err(error) = store.remember_closed(session) {
        eprintln!("Could not remember closed tabs: {error}");
        return;
    }
    if let Err(error) = store.save() {
        eprintln!("Could not save closed tabs: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_windows_and_searchable_dated_visits() -> Result<(), String> {
        let directory = std::env::temp_dir().join(format!(
            "browser-data-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("session.json");
        let mut store = Store::load(path.clone())?;
        store.record("https://example.com/a", "日本語 Rust");
        store.record("about:blank", "Blank");
        store.record("file:///private/test", "Private");
        store.data.windows.push(SessionWindow {
            id: String::new(),
            tabs: vec![
                SessionTab {
                    url: "https://example.com/a".into(),
                    title: "Document".into(),
                    interaction: vec![1, 2, 3],
                },
                SessionTab {
                    url: "about:blank".into(),
                    title: String::new(),
                    interaction: Vec::new(),
                },
            ],
            active: 1,
            is_window: true,
            closed_at: 0,
            position: None,
        });
        store.data.closed = vec![
            SessionWindow::default(),
            SessionWindow {
                id: "closed-1".into(),
                ..Default::default()
            },
            SessionWindow {
                id: "closed-1".into(),
                ..Default::default()
            },
        ];
        store.save()?;
        let restored = Store::load(path.clone())?;
        let ids: std::collections::HashSet<_> = restored
            .data
            .closed
            .iter()
            .map(|entry| entry.id.clone())
            .collect();
        assert_eq!(ids.len(), 3);
        assert!(!ids.contains(""));
        restored.save()?;
        let reloaded = Store::load(path)?;
        assert_eq!(reloaded.data.closed[0].id, restored.data.closed[0].id);
        assert_eq!(restored.data.windows[0].active, 1);
        assert_eq!(restored.data.windows[0].tabs.len(), 2);
        assert_eq!(restored.query("RUST").len(), 1);
        assert_eq!(restored.query("日本語").len(), 1);
        assert_eq!(restored.query("").len(), 1);
        assert!(restored.query("")[0].visited_at > 0);
        fs::remove_dir_all(directory).map_err(|error| error.to_string())
    }

    #[test]
    fn private_windows_are_excluded_from_session_snapshots() {
        let tab = crate::tabs::Tab {
            label: "test".into(),
            title: "Document".into(),
            url: "https://example.com/".into(),
            interaction: vec![9],
        };
        let mut windows = crate::tabs::Windows::default();
        windows.entries.insert(
            "main".into(),
            crate::tabs::Tabs {
                entries: vec![tab.clone()],
                active: "test".into(),
                ..Default::default()
            },
        );
        windows.entries.insert(
            "private".into(),
            crate::tabs::Tabs {
                entries: vec![tab],
                persistent: false,
                ..Default::default()
            },
        );
        let restored = sessions(&windows);
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].tabs[0].interaction, vec![9]);
        assert_eq!(restored[0].active, 0);
    }

    #[test]
    fn startup_checkpoints_preserve_the_pending_session() {
        let session = SessionWindow {
            tabs: vec![SessionTab {
                url: "https://example.com/document".into(),
                title: "Saved document".into(),
                interaction: vec![1, 7, 9],
            }],
            ..Default::default()
        };
        let mut windows = crate::tabs::Windows::default();
        windows.entries.insert(
            "main".into(),
            crate::tabs::Tabs {
                pending_restore: Some(session),
                ..Default::default()
            },
        );
        let restored = sessions(&windows);
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].tabs[0].url, "https://example.com/document");
        assert_eq!(restored[0].tabs[0].interaction, vec![1, 7, 9]);
    }

    #[test]
    fn partial_startup_cannot_replace_the_durable_multiwindow_session() -> Result<(), String> {
        let directory = std::env::temp_dir().join(format!(
            "browser-startup-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("session.json");
        let mut store = Store::load(path.clone())?;
        let first = SessionWindow {
            tabs: vec![SessionTab {
                url: "https://example.com/first".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let second = SessionWindow {
            tabs: vec![SessionTab {
                url: "https://example.com/second".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        store.replace_windows(vec![first.clone(), second.clone()])?;
        store.restoring = true;
        store.replace_windows(vec![first.clone()])?;
        assert_eq!(store.data.windows.len(), 2);
        let after_interruption = Store::load(path.clone())?;
        assert_eq!(after_interruption.data.windows.len(), 2);
        assert_eq!(
            after_interruption.data.windows[1].tabs[0].url,
            second.tabs[0].url
        );
        store.restoring = false;
        store.replace_windows(vec![first, second])?;
        assert_eq!(Store::load(path)?.data.windows.len(), 2);
        fs::remove_dir_all(directory).map_err(|error| error.to_string())
    }

    #[test]
    fn closed_ids_survive_list_changes_and_cannot_target_a_new_item() -> Result<(), String> {
        let mut store = Store::default();
        store.remember_closed(SessionWindow::default())?;
        let first_id = store.data.closed[0].id.clone();
        store.remember_closed(SessionWindow::default())?;
        let selected_id = store.data.closed[1].id.clone();
        store.remember_closed(SessionWindow::default())?;
        let latest_id = store.data.closed[2].id.clone();
        store.remove_closed(&first_id)?;
        let selected = store
            .data
            .closed
            .iter()
            .find(|entry| entry.id == selected_id)
            .unwrap()
            .clone();
        assert_eq!(selected.id, selected_id);
        store.remove_closed(&selected.id)?;
        assert_eq!(store.data.closed.len(), 1);
        assert_eq!(store.data.closed[0].id, latest_id);
        assert!(store.remove_closed(&selected_id).is_err());
        store.data.closed.clear();
        store.remember_closed(SessionWindow::default())?;
        assert_ne!(store.data.closed[0].id, selected_id);
        assert!(store.remove_closed(&selected_id).is_err());
        assert_eq!(store.data.closed.len(), 1);
        Ok(())
    }

    #[test]
    fn corrupt_storage_is_not_silently_overwritten() -> Result<(), String> {
        let path =
            std::env::temp_dir().join(format!("browser-corrupt-{}.json", std::process::id()));
        fs::write(&path, "incomplete").map_err(|error| error.to_string())?;
        assert!(Store::load(path.clone()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "incomplete");
        fs::remove_file(path).map_err(|error| error.to_string())
    }
}
