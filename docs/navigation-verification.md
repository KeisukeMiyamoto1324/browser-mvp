# Native navigation verification

Scope: #1. Tested on macOS 27.0.1 (26A434), Apple Silicon, using the debug app bundle. This is functional evidence, not proof of full visual or motion parity.

Run `node --experimental-strip-types tools/navigation-fixture.ts`, build with `npm run tauri build -- --debug --bundles app`, and open the debug bundle. Visit `http://127.0.0.1:18765/one`.

| Check | Observed result |
| --- | --- |
| Initial application launch | Back and Forward disabled while the initial website loads |
| Two-page navigation | Back enabled; Forward disabled at the newest entry |
| Replace page history methods, then Back | Returns to page one using native navigation |
| Forward after Back | Returns to page two; Forward becomes disabled |
| Reload | Request counter changes from 2 to 3 |
| Same-document anchor, then Back | Address changes to `#section`, then removes the fragment; Forward enabled |
| Redirect after going Back | Final address is `/two`; Forward disabled |
| Slow response | Stop loading replaces Reload; blue progress line visible |
| Stop during slow response | Reload returns without waiting for the 30-second response |
| Blank page, then Back | Address shows `about:blank`, then returns to `/one`; Forward enabled |

Paired local reference/application screenshots were captured for page one, Back, loading, and stopped states under `/tmp/browser-interface-review`. The reference showed the same Back/Forward availability and Stop/Reload state changes for these operations. Captures contain personal toolbar items and remain local.

The captures have different window dimensions. Toolbar grouping, title-bar height, address presentation, materials and geometry still differ and belong to #2. Full motion comparison, all menus and dialogs, error-page behavior, light-mode captures and exact dimensions remain unverified. Do not treat this record as visual sign-off. #10 tracks the complete comparison inventory.

Local validation passed: `npm run format`, `npm run lint`, `npm run check`, `npm run build`, `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (3 address-resolution tests), and debug app bundling. Rust unit tests do not exercise WKWebView; the table above records native UI checks. A macOS CI workflow runs the source checks for each PR.
