# Tab pinning and ordering verification

Verified on macOS on 2026-10-24 with the debug app built in the issue-21 worktree.
The disposable fixture runs with `node --experimental-strip-types tools/tab-fixture.ts`
and serves `http://127.0.0.1:18768/first`. It uses `localhost` as a second host.

| Check | Observed result |
| --- | --- |
| Native Pin / Unpin menu | Pin produces a 42-point control without a close button; unpin restores a regular tab. The document stays open. |
| Background pin and move | The active document and its input remain selected. |
| Move Tab Left / Right | The order changes within the corresponding pinned or regular group. Boundary actions are disabled. |
| Regular-tab drag | Dragging the second tab onto the first changes the AX order and preserves the active document. |
| Pinned-tab drag | Dragging the second pinned control onto the first changes the pinned order and preserves selection. |
| History after pin, unpin and reorder | A same-host navigation remains in the pinned tab. After unpin and reorder, Back restores the first page and the input `keep-pinned-note`. |
| Cross-host top-level link | A regular tab opens at `localhost`; the pinned page at `127.0.0.1`, its input and its iframe remain available. |
| Cross-origin iframe | Adding an iframe after pinning loads it inside the existing page without creating another tab. |
| Regular overflow | The horizontal scroll area reaches both ends; pinned controls stay visible in their separate region. |

The first bundle reproduced an iframe defect: Wry 0.57.1's URL-only
`navigation_policy` invokes the navigation handler without checking `targetFrame`.
The corrected bundle installs a retained native delegate proxy. It checks
`WKNavigationAction.targetFrame.isMainFrame` before diverting pinned links and
forwards existing loading, navigation and history callbacks to Wry's delegate.
The same fixture then loaded its cross-origin iframe in place.

Screenshots of the overflow strip and the retained input were inspected in the
computer-use session. The final pinned-order capture is stored locally at
`/tmp/browser-interface-review/app-tab-pinned-order.png`. They contain only the disposable fixture and development app.
No personal reference windows were operated during this verification.

Automated validation passed: npm format, check, lint and build; cargo fmt;
cargo test --lib (8 tests); cargo clippy --all-targets with warnings denied;
and a debug app bundle build. Native drag succeeded through the current
computer-use API; no substitute event-synthesis tool was used.

Remaining work: integrate pinned URL, flag and order into session/closed-tab
restoration; verify the integrated popup path installs the same delegate;
and compare timing and motion against controlled reference windows.
The dragged cell follows the pointer, but adjacent cells have no animated
insertion transition. Pinned controls currently display the first title letter.

## Request-method preservation follow-up

Only bodyless GET requests may use URL-only pinned-site diversion. POST and
requests with body data or a body stream are forwarded to the original native
navigation delegate without cancelling or reconstructing their request. A form
may therefore navigate the pinned view to another host; preserving the form
method and payload takes priority over URL-only diversion.

The tab fixture includes **Submit other-host POST**. Pin the page at
`http://127.0.0.1:18768/first`, submit it, and check the destination and
`http://127.0.0.1:18768/submissions`. The received method must be POST, the fixture
field must be `preserve-post-body`, and the message must retain its spaces,
plus sign and ampersand. A GET at `/submitted` is a failure. Also repeat with an
ordinary cross-host GET link to verify that safe link diversion still works.

| UI/state | Reference capture | App capture | Comparison | Interaction/motion |
| --- | --- | --- | --- | --- |
| Pinned cross-host form before submission | Not captured | Not captured | Unverified | Unverified |
| POST destination and retained body in the same view | Not captured | Not captured | Unverified | Unverified |
| Cross-host GET in a regular tab; source pin retained | Not captured | Not captured | Unverified | Unverified |

The native request policy has an automated test for default GET, POST, body
and body-stream requests. CLI fixture checks do not establish browser runtime
or visual equivalence; paired screenshots and interactions remain required.
