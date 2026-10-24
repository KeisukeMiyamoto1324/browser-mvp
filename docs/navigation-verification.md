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

### Review follow-up and size recovery

The automated review of 5933c2e found that repeated Command-L overwrote an active edit and Escape ended editing without focusing content. Repeated Command-L now selects the existing field editor without making it resign first responder. Escape first restores a changed address, then returns focus to content; Tab and Return then activated the first page link without a mouse click. The reference additionally dismisses its suggestions before reverting the draft; that suggestion stage remains pending in #4.

A later native check exposed permanent address overflow after leaving full screen. Width updates previously depended on the detached field having a window. They now read a retained owner window, and captures confirm that the address field returns after full-screen exit and native Zoom/restore. The app Zoom capture changed from 6016 × 3234 to 2200 × 1520 on restore. Reference Zoom sizing differs and is tracked in #14; these states are not visually equivalent.

One immediate post-transition address-entry attempt produced an unsupported-scheme alert; the submitted text was not captured. After the transition settled, input was inspected before submission and correctly opened `/two`. Repeat rapid transition-time entry before sign-off rather than treating the settled-state check as proof for that timing case. All small-window, light-mode, missing-control and motion limits above remain open.

## Isolated tab foundation (issue #3, work in progress)

The `issue-3-tab-lifecycle` branch builds on the native toolbar branch. Each tab owns a separate child WebView. The active tab drives navigation controls; stale queued polls from an inactive tab are discarded. Native title-bar accessories provide the initial tab selector, new-tab button and active-tab close button.

Local macOS interaction checks confirmed that switching retained the original page request number, input text, and independent Back/Forward availability. Command-T created a blank tab with address focus; Command-W closed only the active tab; Command-Shift-T reopened its destination; Control-Tab and Control-Shift-Tab changed selection. A target-blank fixture link opened a separate tab. Reopening a closed window recreated its native controls. Command-Q still terminated the process. The fixture now includes a tab note and a separate-tab link for these checks.

Two implementation faults were found and fixed during testing: reading a new WebView URL before WebKit assigned it caused a startup panic, and mutating WebViews inside a native callback could deadlock the dispatcher's window lock. URL reads now handle an absent native URL, and content mutations happen after the callback returns. The default Window menu also had a second Command-W binding; it now uses Command-Shift-W for Close Window.

Local captures `reference-two-navigation-tabs.png`, `app-two-tabs.png`, `app-three-tabs.png` and `app-tabs-size-transition.png` document current states. They are not visual sign-off: window sizes differ, the tab selector's geometry/material/labels and close placement still differ, and the reference favorites bar is not implemented. Captures remain local because the reference includes personal toolbar contents.

Issue #3 remains open. Closed tabs currently restore only their destination, without their previous document, scroll position or navigation stack. Reordering, pinning, groups, overview, per-tab close affordances, overflow, multiple simultaneous windows, and full motion/size validation remain unfinished. New tabs use a blank page until issue #4 supplies the start page. Rapid unobserved input batches also need a controlled comparison; stepwise address entry was inspected before submission. This is a foundation for review, not completion of the tab/window interface.
