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
}

#[derive(Default)]
pub struct Store {
    pub data: Data,
    pub path: PathBuf,
    pub quitting: bool,
}

impl Store {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        let data = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| error.to_string())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Data::default(),
            Err(error) => return Err(error.to_string()),
        };
        Ok(Self {
            data,
            path,
            quitting: false,
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
    if store.quitting {
        return;
    }
    store.data.windows = sessions;
    if let Err(error) = store.save() {
        eprintln!("Could not save browser data: {error}");
    }
}

pub fn closed(app: &AppHandle, mut session: SessionWindow) {
    if session.tabs.is_empty() {
        return;
    }
    let state = app.state::<Mutex<Store>>();
    let Ok(mut store) = state.lock() else { return };
    if store.quitting {
        return;
    }
    session.closed_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    store.data.closed.push(session);
    if store.data.closed.len() > 20 {
        store.data.closed.remove(0);
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
        store.save()?;
        let restored = Store::load(path)?;
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
    fn corrupt_storage_is_not_silently_overwritten() -> Result<(), String> {
        let path =
            std::env::temp_dir().join(format!("browser-corrupt-{}.json", std::process::id()));
        fs::write(&path, "incomplete").map_err(|error| error.to_string())?;
        assert!(Store::load(path.clone()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "incomplete");
        fs::remove_file(path).map_err(|error| error.to_string())
    }
}
