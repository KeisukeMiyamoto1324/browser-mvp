use tauri::{LogicalPosition, LogicalSize, Manager, Webview, WebviewUrl};

const TOOLBAR_HEIGHT: f64 = 64.0;

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
        return match url.scheme() {
            "http" | "https" if url.host_str().is_some() => Ok(url),
            _ => Err("Only HTTP and HTTPS addresses are supported.".into()),
        };
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
    if toolbar.label() != "main" {
        return Err("Browser controls are only available to the toolbar.".into());
    }
    toolbar
        .app_handle()
        .get_webview("content")
        .ok_or_else(|| "The browser WebView is not ready.".into())
}

#[tauri::command]
async fn resize_content(webview: Webview, viewport_height: f64) -> Result<(), String> {
    let content = browser_content(&webview)?;
    let bounds = content_bounds(&webview.window(), viewport_height).map_err(|e| e.to_string())?;
    content
        .set_bounds(bounds)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn navigate(webview: Webview, address: String) -> Result<(), String> {
    browser_content(&webview)?
        .navigate(resolve_address(&address)?)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn browser_url(webview: Webview) -> Result<String, String> {
    browser_content(&webview)?
        .url()
        .map(|url| url.to_string())
        .map_err(|error| error.to_string())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Direction {
    Back,
    Forward,
}

#[tauri::command]
async fn navigate_history(webview: Webview, direction: Direction) -> Result<(), String> {
    let script = match direction {
        Direction::Back => "window.history.back()",
        Direction::Forward => "window.history.forward()",
    };
    browser_content(&webview)?
        .eval(script)
        .map_err(|error| error.to_string())
}

// ---------------------------------------------------------
// Keep remote content below the fixed-height local toolbar.
// ---------------------------------------------------------
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let window = app.get_window("main").ok_or("Main window was not found.")?;
            let bounds = content_bounds(&window, 0.0)?;
            let handle = app.handle().clone();
            let content = tauri::webview::WebviewBuilder::new(
                "content",
                WebviewUrl::External("https://duckduckgo.com/".parse()?),
            )
            .on_navigation(|url| matches!(url.scheme(), "http" | "https"))
            .on_new_window(move |url, _features| {
                if matches!(url.scheme(), "http" | "https")
                    && let Some(content) = handle.get_webview("content")
                {
                    let _ = content.navigate(url);
                }
                tauri::webview::NewWindowResponse::Deny
            });
            window.add_child(content, bounds.position, bounds.size)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            resize_content,
            navigate,
            browser_url,
            navigate_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running browser");
}

#[cfg(test)]
mod tests {
    use super::resolve_address;

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
