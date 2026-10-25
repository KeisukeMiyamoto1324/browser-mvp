use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager, Webview};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub title: String,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReadingItem {
    pub id: u64,
    pub title: String,
    pub url: String,
    pub read: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPages {
    pub bookmarks: Vec<Bookmark>,
    pub reading_list: Vec<ReadingItem>,
    next_id: u64,
}

pub struct Store {
    path: PathBuf,
    pages: SavedPages,
}

impl Store {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        let pages = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| error.to_string())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => SavedPages::default(),
            Err(error) => return Err(error.to_string()),
        };
        Ok(Self { path, pages })
    }

    // ---------------------------------------------------------
    // Write a complete replacement before changing live data.
    // A failed save leaves the current in-memory library intact.
    // ---------------------------------------------------------
    fn replace(&mut self, pages: SavedPages) -> Result<SavedPages, String> {
        let parent = self.path.parent().ok_or("Invalid library path.")?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(&pages).map_err(|error| error.to_string())?;
        use std::io::Write;
        let mut file = std::fs::File::create(&temporary).map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        std::fs::rename(temporary, &self.path).map_err(|error| error.to_string())?;
        self.pages = pages.clone();
        Ok(pages)
    }
}

pub fn snapshot(app: &AppHandle) -> Result<SavedPages, String> {
    Ok(app
        .state::<Mutex<Store>>()
        .lock()
        .map_err(|error| error.to_string())?
        .pages
        .clone())
}

#[tauri::command]
pub async fn get_saved_pages(webview: Webview) -> Result<SavedPages, String> {
    crate::browser_content(&webview)?;
    snapshot(webview.app_handle())
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Edit {
    Bookmark {
        id: Option<u64>,
        #[serde(rename = "parentId")]
        parent_id: Option<u64>,
        title: String,
        url: Option<String>,
    },
    Reading {
        id: Option<u64>,
        title: String,
        url: String,
        read: bool,
    },
    DeleteBookmark {
        id: u64,
    },
    DeleteReading {
        id: u64,
    },
}

// ---------------------------------------------------------
// Validate destinations and folder ancestry before persisting.
// Deleting a folder also removes every descendant.
// ---------------------------------------------------------
fn apply(pages: &mut SavedPages, edit: Edit) -> Result<(), String> {
    match edit {
        Edit::Bookmark {
            id,
            parent_id,
            title,
            url,
        } => {
            if title.trim().is_empty() {
                return Err("Enter a name.".into());
            }
            if let Some(url) = &url {
                validate_url(url)?;
            }
            if let Some(parent) = parent_id {
                let mut ancestor = Some(parent);
                while let Some(current) = ancestor {
                    if Some(current) == id {
                        return Err("A folder cannot contain itself.".into());
                    }
                    let folder = pages
                        .bookmarks
                        .iter()
                        .find(|item| item.id == current && item.url.is_none())
                        .ok_or("The folder is no longer available.")?;
                    ancestor = folder.parent_id;
                }
            }
            if let Some(id) = id {
                let item = pages
                    .bookmarks
                    .iter_mut()
                    .find(|item| item.id == id)
                    .ok_or("The bookmark is no longer available.")?;
                if item.url.is_none() != url.is_none() {
                    return Err("The item type cannot change.".into());
                }
                *item = Bookmark {
                    id,
                    parent_id,
                    title: title.trim().into(),
                    url,
                };
            } else {
                pages.next_id += 1;
                pages.bookmarks.push(Bookmark {
                    id: pages.next_id,
                    parent_id,
                    title: title.trim().into(),
                    url,
                });
            }
        }
        Edit::Reading {
            id,
            title,
            url,
            read,
        } => {
            validate_url(&url)?;
            if title.trim().is_empty() {
                return Err("Enter a name.".into());
            }
            let existing = id.or_else(|| {
                pages
                    .reading_list
                    .iter()
                    .find(|item| item.url == url)
                    .map(|item| item.id)
            });
            if let Some(id) = existing {
                let item = pages
                    .reading_list
                    .iter_mut()
                    .find(|item| item.id == id)
                    .ok_or("The reading item is no longer available.")?;
                *item = ReadingItem {
                    id,
                    title: title.trim().into(),
                    url,
                    read,
                };
            } else {
                pages.next_id += 1;
                pages.reading_list.push(ReadingItem {
                    id: pages.next_id,
                    title: title.trim().into(),
                    url,
                    read,
                });
            }
        }
        Edit::DeleteBookmark { id } => {
            let mut removed = vec![id];
            loop {
                let children: Vec<u64> = pages
                    .bookmarks
                    .iter()
                    .filter(|item| {
                        item.parent_id
                            .is_some_and(|parent| removed.contains(&parent))
                            && !removed.contains(&item.id)
                    })
                    .map(|item| item.id)
                    .collect();
                if children.is_empty() {
                    break;
                }
                removed.extend(children);
            }
            pages.bookmarks.retain(|item| !removed.contains(&item.id));
        }
        Edit::DeleteReading { id } => pages.reading_list.retain(|item| item.id != id),
    }
    Ok(())
}

fn validate_url(input: &str) -> Result<(), String> {
    let url = tauri::Url::parse(input).map_err(|error| error.to_string())?;
    crate::is_web_address(&url)
        .then_some(())
        .ok_or("Only web addresses can be saved.".into())
}

#[tauri::command]
pub async fn edit_saved_pages(webview: Webview, edit: Edit) -> Result<SavedPages, String> {
    crate::browser_content(&webview)?;
    let state = webview.state::<Mutex<Store>>();
    let mut store = state.lock().map_err(|error| error.to_string())?;
    let mut pages = store.pages.clone();
    apply(&mut pages, edit)?;
    let pages = store.replace(pages)?;
    drop(store);
    let _ = webview.app_handle().emit("saved-pages-changed", ());
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_folders_reading_state_and_deletes_descendants() -> Result<(), String> {
        let path = std::env::temp_dir().join(format!(
            "browser-saved-pages-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut store = Store::load(path.clone())?;
        let mut pages = store.pages.clone();
        apply(
            &mut pages,
            Edit::Bookmark {
                id: None,
                parent_id: None,
                title: "Folder".into(),
                url: None,
            },
        )?;
        apply(
            &mut pages,
            Edit::Bookmark {
                id: None,
                parent_id: Some(1),
                title: "Page".into(),
                url: Some("https://example.com/".into()),
            },
        )?;
        assert!(
            apply(
                &mut pages,
                Edit::Bookmark {
                    id: Some(1),
                    parent_id: Some(1),
                    title: "Cycle".into(),
                    url: None
                }
            )
            .is_err()
        );
        assert!(
            apply(
                &mut pages,
                Edit::Reading {
                    id: None,
                    title: "Invalid".into(),
                    url: "file:///tmp/a".into(),
                    read: false
                }
            )
            .is_err()
        );
        apply(
            &mut pages,
            Edit::Reading {
                id: None,
                title: "Read".into(),
                url: "https://example.com/".into(),
                read: false,
            },
        )?;
        apply(
            &mut pages,
            Edit::Reading {
                id: None,
                title: "Read".into(),
                url: "https://example.com/".into(),
                read: true,
            },
        )?;
        store.replace(pages)?;
        let mut loaded = Store::load(path.clone())?.pages;
        assert_eq!(loaded.bookmarks.len(), 2);
        assert_eq!(loaded.reading_list.len(), 1);
        assert!(loaded.reading_list[0].read);
        apply(&mut loaded, Edit::DeleteBookmark { id: 1 })?;
        assert!(loaded.bookmarks.is_empty());
        std::fs::remove_file(path).map_err(|error| error.to_string())?;
        Ok(())
    }
}
