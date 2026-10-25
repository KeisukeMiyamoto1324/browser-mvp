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

The `issue-3-tab-lifecycle` branch builds on the native toolbar branch. Each tab owns a separate child WebView. The active tab drives navigation controls; stale queued polls from an inactive tab are discarded. Native title-bar accessories provide individual tab controls, per-tab close buttons and horizontal overflow. The new-tab action is in the native toolbar.

Local macOS interaction checks confirmed that switching retained the original page request number, input text, and independent Back/Forward availability. Command-T created a blank tab with address focus; Command-W closed only the active tab; Command-Shift-T reopened its destination; Control-Tab and Control-Shift-Tab changed selection. A target-blank fixture link opened a separate tab. Reopening a closed window recreated its native controls. Command-Q still terminated the process. The fixture now includes a tab note and a separate-tab link for these checks.

Two implementation faults were found and fixed during testing: reading a new WebView URL before WebKit assigned it caused a startup panic, and mutating WebViews inside a native callback could deadlock the dispatcher's window lock. URL reads now handle an absent native URL, and content mutations happen after the callback returns. The default Window menu also had a second Command-W binding; it now uses Command-Shift-W for Close Window.

Local captures `reference-two-navigation-tabs.png`, `app-two-tabs.png`, `app-three-tabs.png` and `app-tabs-size-transition.png` document current states. They are not visual sign-off: window sizes differ, the tab selector's geometry/material/labels and close placement still differ, and the reference favorites bar is not implemented. Captures remain local because the reference includes personal toolbar contents.

Issue #3 remains open. Closed tabs currently restore only their destination, without their previous document, scroll position or navigation stack. Reordering, pinning, groups, overview, close-button hover behavior, overflow navigation details, multiple simultaneous windows, and full motion/size validation remain unfinished. New tabs use a blank page until issue #4 supplies the start page. Rapid unobserved input batches also need a controlled comparison; stepwise address entry was inspected before submission. This is a foundation for review, not completion of the tab/window interface.


### Individual tab controls and overflow follow-up

The segmented selector was replaced with glass tab controls using stable WebView labels. Both active and inactive tabs can be closed directly. Closing an inactive tab left the current page and its request number unchanged; reopening it restored the correct destination. Eleven-tab checks confirmed horizontal scrolling and keyboard wrapping between the first and last tab. Selected tabs are revealed after selection, size or collection changes. Native scroll-style changes initially reserved space and clipped labels; the tab scroll view now keeps overlay scrollers so content retains its full height. The final left-scroll capture shows readable labels and the scroll position changed from 1 to 0.

Additional local captures include `app-individual-tabs.png`, `app-fixed-overlay-overflow-left.png`, `app-fixed-overlay-overflow-active.png`, `reference-navigation-tabs-fullscreen.png` and `app-individual-tabs-fullscreen-settled.png`. Settled full-screen captures are both 6016 × 3384. The immediate content rectangle can lag behind the native full-screen transition; #16 tracks this separately. The intermediate capture is not a matching result. Toolbar controls, favorites, tab labels/icons, hover appearance, overflow details and motion still differ, so #2, #3 and #10 remain open and both interface PRs remain drafts.

## Native content resizing follow-up (issue #16, work in progress)

The `issue-16-native-content-sizing` branch enables native width/height autoresizing on every content WebView. New tabs read the active WebView's current native bounds instead of copying the last frontend resize result. Wry otherwise creates child WebViews with fixed dimensions, so their size previously remained unchanged until a later frontend callback.

A thresholded white-pixel measurement and visual inspection of immediate full-screen captures confirmed the specific improvement. Before the change, a 6016 × 3384 capture had a white-pixel extent of `(0, 180, 2200, 1520)`. After the change that extent was `(0, 180, 6016, 3384)`. The fixture page reached the right and bottom edges immediately. A second full-screen entry reproduced the corrected extent, and a new tab opened at full-screen size. This image check does not record native view geometry. The hidden first tab also filled the full-screen window when selected. Exit and settled address-entry checks succeeded; the inspected `/sizing-three` address opened the intended destination.

Local evidence: `app-autoresize-fullscreen-immediate.png`, `app-autoresize-hidden-tab-fullscreen.png`, and `app-autoresize-exit-immediate.png`. These do not establish full animation equivalence. The native header can still be in an intermediate transition state, and an immediate Command-L batch during exit did not focus the address field. The matching reference timing check was interrupted when its active window changed; other reference tabs were not edited. Transition-time input comparison remains pending, along with the broader #2/#10 visual and motion work.

## Independent window foundation (issue #18, work in progress)

The `issue-18-independent-windows` branch replaces the single tab collection and native control slots with per-window state. Each local toolbar is authorized only for its registered owning window. New Window uses Command-N; close, tab and navigation actions route to the active window, while background polling updates only that window's native controls. Window titles follow the active page. Initial and new windows use a blank placeholder until #4 supplies the start page. Initial address focus is applied after the local toolbar mounts, when the native field is attached.

Native checks opened two distinct fixture windows. The first retained request 26 while the second retained request 27. Adding a second tab to the first produced request 28 without changing the second window's tab count or request number. Back in the second returned to its original request. Closing the second preserved both tabs and the `first window note` input in the first. The native Window menu listed the distinct page titles and selected the intended window.

A later run opened a 700 × 500 requested popup, captured at 1400 × 1000 physical pixels. A target-blank link added a second tab inside that popup. Full-screen entry/exit in a third window left the popup at 1400 × 1000 with both tabs and its original request number. Closing all three windows in sequence selected the correct surviving windows; reactivating after the last close recreated a blank window with address focus. Quit was also checked. A new authorization test rejects child views, another window's toolbar, unregistered labels and removed windows; all four Rust tests passed, along with formatting, lint and app build checks.

Local captures include `app-window-one-two-tabs.png`, `app-window-two-one-tab.png`, `app-popup-window.png`, `app-popup-window-two-tabs.png`, `app-third-window-fullscreen.png` and `app-popup-after-other-window-fullscreen.png`. These are interaction evidence, not full visual sign-off. The corresponding reference timing capture was interrupted by active-window changes, so it was not retried against unrelated tabs. Command-grave/tilde did not switch windows in the automation; its cause and matching reference behavior remain unverified. The Window menu did switch windows successfully.

Tab transfer/merge, complete window restoration, pinned/group state and full visual/motion checks remain open under #3/#6/#10. Popup requests still recreate the destination and return a denied native response; scripting/opener relationships are not preserved. This foundation does not establish complete multi-window equivalence.

### Shared Stop menu review follow-up

Automated review of #19 found that per-window loading updates could overwrite the shared Stop menu and did not resynchronize unchanged loading states after a focus change. Native verification also found that the previous root-level menu lookup did not reach the nested View item. The application now retains the actual Stop item, updates it only from the key window, synchronizes it on focus changes, and disables it after the last window closes.

The `/slow-window` fixture keeps a response open long enough to inspect native menus without racing completion. With a loading window behind an idle blank window, the idle window's Stop item was disabled. Returning to the loading window made Stop enabled even though its loading state had not changed. Command-period stopped that request and restored the Reload control. These checks passed after the fix, alongside Biome checks, rustfmt, Clippy, four Rust tests and the app build. A fresh automated review and CI run are required for the follow-up commit.
