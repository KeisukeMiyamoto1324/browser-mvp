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

## Native window toolbar work in progress (#2)

The `issue-2-native-window-toolbar` branch attaches an AppKit `NSToolbar` to the existing Tauri window, using native segmented navigation controls, system symbols, a centered glass address field, Reload/Stop, and progress. React continues to synchronize navigation state and content bounds; the HTML toolbar is hidden on macOS. This branch requires macOS 26 or newer for the glass view.

Native UI checks on macOS 27.0.1 confirmed mouse and Command-L address entry, Return navigation, Back/Forward after page-script overrides, Command-R reload, loading cancellation, Escape cancellation, and full-screen entry/exit. An address-editing bug that left part of the previous URL was found and fixed by expanding the URL before the field becomes first responder. A repeated-focus guard preserves an existing edit. A final native check typed `/two`, switched to the reference app, then returned and submitted `/two` successfully.

Local paired captures include the reference address-editing state and application native toolbar/editing/loading/full-screen states. Screenshots showed a rectangular focus ring, oversized loading track, and a capped address width in full screen; these were adjusted. Full-screen captures use the same 6016 × 3384 display; normal-window dimensions still differ. A corner-drag attempt did not change the app dimensions and is not a passed resize test.

The reference has additional sidebar, group, page, translation, sharing and new-tab controls plus a favorites bar. These features remain absent and tracked in #3–#7. Address editing still includes the URL scheme, and suggestion panels are not implemented (#4). Light-mode appearance, small-window resizing, exact typography/colors, toolbar overflow, all menu/dialog states, and motion timing still need comparison. Full-screen entry/exit was exercised, but screenshots alone do not prove animation parity. #2 and #10 remain open; there is no visual sign-off.
