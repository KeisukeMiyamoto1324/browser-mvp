# Interface comparison coverage

This inventory covers issues #2 through #10, including work that is not implemented.
It describes the integration branch after the start-page, history, popup,
and pinning changes. Find/page actions and downloads remain separate work until
explicitly integrated. Sidebar UI is not planned per user request; the shared
saved-page data API remains for Favorites. Implementation and passing CLI/runtime checks do not mean
the interface matches the reference. No row below has visual sign-off.

Each row requires paired reference/application screenshots of the same state,
window content size, appearance, display scale and fixture data. Capture each
listed variant separately. Also cover normal, minimum-size and full-screen
windows; active/inactive windows; light/dark appearance; and enabled/disabled,
focus, hover, pressed, keyboard and pointer states wherever they apply. Motion
requires matched recordings or before/during/after frames, including timing.
Every menu must be expanded into its complete observed item inventory, including
submenus, separators, shortcuts, checkmarks and disabled items. Add rows for every
new dialog, alert, sheet, context menu or state discovered during comparison.

Reference captures, application screenshots, recordings and personal browsing
information stay local. This file contains no images or private evidence paths.
A future row may use a neutral local evidence ID; that does not imply comparison
success. Missing captures remain **Not captured**, and mismatches remain open.
The comparison owner records the actual difference and follow-up issue before
changing a result. Changes to #7/#8 can append rows using the same IDs/columns.

| ID | UI/state and implementation | Reference capture | App capture | Comparison | Interaction/motion | Issue |
| --- | --- | --- | --- | --- | --- | --- |
| T02-01 | Native title bar and traffic lights: active, inactive, hover. Integrated. | Not captured | Not captured | Unverified | Unverified | #2 |
| T02-02 | Toolbar: empty page, normal page, disabled history buttons. Integrated. | Not captured | Not captured | Unverified | Unverified | #2 |
| T02-03 | Loading progress, Stop, Reload, back and forward availability. Integrated. | Not captured | Not captured | Unverified | Unverified | #2 |
| T02-04 | Address field: idle, focus ring, selected text, edited draft. Integrated. | Not captured | Not captured | Unverified | Unverified | #2, #4 |
| T02-05 | Repeated location shortcut, Return, Escape, content focus. Integrated. | Not captured | Not captured | Unverified | Unverified | #2, #4 |
| T02-06 | Invalid or unsupported address alert: buttons and dismissal. Integrated native alert path. | Not captured | Not captured | Unverified | Unverified | #2, #9 |
| T02-07 | Window resize: minimum, normal, wide, restored dimensions. Integrated. | Not captured | Not captured | Unverified | Unverified | #2, #10 |
| T02-08 | Minimize, native Zoom and restore, full-screen entry and exit. Integrated; sizing differences remain open. | Not captured | Not captured | Unverified | Unverified | #2, #14, #16 |
| T02-09 | Toolbar material, separator, typography and icon spacing. Integrated. | Not captured | Not captured | Unverified | Unverified | #2 |
| T03-01 | Single tab: active title, close control, new-tab action. Integrated. | Not captured | Not captured | Unverified | Unverified | #3 |
| T03-02 | Multiple tabs: active, inactive, hover, close visibility. Integrated. | Not captured | Not captured | Unverified | Unverified | #3 |
| T03-03 | Tab switching: pointer, keyboard, retained document and history. Integrated. | Not captured | Not captured | Unverified | Unverified | #3 |
| T03-04 | Tab overflow: horizontal scroll and narrow window. Integrated. | Not captured | Not captured | Unverified | Unverified | #3 |
| T03-05 | Pinned tabs: selected, unselected, compact width, overflow. Integrated. | Not captured | Not captured | Unverified | Unverified | #3, #21 |
| T03-06 | Tab context menu: pin, unpin, move left/right, disabled limits. Integrated. | Not captured | Not captured | Unverified | Unverified | #3, #21 |
| T03-07 | Tab drag: regular and pinned groups, insertion and cancel. Integrated. | Not captured | Not captured | Unverified | Unverified | #3, #21 |
| T03-08 | Close active/background/last tab; reopen pinned/regular tab. Integrated. | Not captured | Not captured | Unverified | Unverified | #3, #6, #21 |
| T03-09 | Independent windows: create, focus, close, menu ownership. Integrated. | Not captured | Not captured | Unverified | Unverified | #3, #18 |
| T03-10 | Script popup: size, source ownership, native toolbar, close. Integrated remote-only window. | Not captured | Not captured | Unverified | Unverified | #3, #20 |
| T03-11 | Popup History routing to a local toolbar window. Integrated; runtime check pending. | Not captured | Not captured | Unverified | Unverified | #6, #20 |
| T03-12 | Tab overview: enter, search, select, close and exit. Not implemented. | Not captured | Not captured | Unverified | Unverified | #3 |
| T03-13 | Tab groups: create, rename, move, select and delete. Not implemented. | Not captured | Not captured | Unverified | Unverified | #3 |
| T03-14 | Tab icons, hover details and audio/media tab controls. Not fully implemented. | Not captured | Not captured | Unverified | Unverified | #3 |
| T04-01 | Start page: Favorites, Recently Visited, Open Tabs populated. Integrated. | Not captured | Not captured | Unverified | Unverified | #4 |
| T04-02 | Start page: empty sections, hidden sections, load/error state. Integrated. | Not captured | Not captured | Unverified | Unverified | #4 |
| T04-03 | Add Favorite dialog: initial, filled, validation, Add, Cancel. Integrated HTML dialog. | Not captured | Not captured | Unverified | Unverified | #4 |
| T04-04 | Customize Start Page: open, toggles, background, outside click. Integrated HTML popover. | Not captured | Not captured | Unverified | Unverified | #4 |
| T04-05 | Address suggestions: empty query, typed query, no results. Integrated native panel. | Not captured | Not captured | Unverified | Unverified | #4 |
| T04-06 | Address suggestions: selected row, arrows, Return, Escape. Integrated native panel. | Not captured | Not captured | Unverified | Unverified | #4 |
| T04-07 | Suggestion panel placement: minimum size and full-screen. Integrated; comparison pending. | Not captured | Not captured | Unverified | Unverified | #4 |
| T04-08 | Start-page navigation and open-tab selection: content focus. Integrated followup; runtime check pending. | Not captured | Not captured | Unverified | Unverified | #4 |
| T05-01 | Sidebar: hidden, open, selected section, empty library. Not planned per user; removed. | Not captured | Not captured | Not planned per user | Not planned per user | #5 |
| T05-02 | Bookmarks: selection, expanded/collapsed nested folders. Not planned per user; removed. | Not captured | Not captured | Not planned per user | Not planned per user | #5 |
| T05-03 | Reading List: unread/read selection, state toggle, empty list. Not planned per user; removed. | Not captured | Not captured | Not planned per user | Not planned per user | #5 |
| T05-04 | Add/Edit Bookmark form: name, address, folder choice. Not planned per user; removed. | Not captured | Not captured | Not planned per user | Not planned per user | #5 |
| T05-05 | Add/Edit Folder form: name, parent, Save and Cancel. Not planned per user; removed. | Not captured | Not captured | Not planned per user | Not planned per user | #5 |
| T05-06 | Add/Edit Reading Item form: name, address, read toggle. Not planned per user; removed. | Not captured | Not captured | Not planned per user | Not planned per user | #5 |
| T05-07 | Saved-page validation/error, delete item/folder descendants. Not planned per user; removed. | Not captured | Not captured | Not planned per user | Not planned per user | #5 |
| T05-08 | Sidebar resizing: pointer, keyboard, minimum/maximum limits. Not planned per user; removed. | Not captured | Not captured | Not planned per user | Not planned per user | #5 |
| T05-09 | Sidebar editor at minimum window height: scrolling and buttons. Not planned per user; removed. | Not captured | Not captured | Not planned per user | Not planned per user | #5 |
| T05-10 | Sidebar with Start Page and History: reserved width and focus. Not planned per user; removed. | Not captured | Not captured | Not planned per user | Not planned per user | #4, #5, #6 |
| T06-01 | History: dated list, empty list, query, no results. Integrated. | Not captured | Not captured | Unverified | Unverified | #6 |
| T06-02 | Recently closed list: tabs, windows, stable selection across windows. Integrated followup; runtime check pending. | Not captured | Not captured | Unverified | Unverified | #6 |
| T06-03 | Clear History dialog: ranges, initial, selected, confirm, cancel. Integrated HTML dialog. | Not captured | Not captured | Unverified | Unverified | #6 |
| T06-04 | History: open destination, Escape, close surface, return focus. Integrated. | Not captured | Not captured | Unverified | Unverified | #6 |
| T06-05 | Restart restore: windows, selection, order, pins, document history. Integrated; runtime restore comparison pending. | Not captured | Not captured | Unverified | Unverified | #6, #21 |
| T07-01 | Application menu: every item, enabled/disabled and shortcuts. Default menu plus integrated actions; exact inventory pending. | Not captured | Not captured | Unverified | Unverified | #7 |
| T07-02 | File menu: every item, enabled/disabled and shortcuts. Partly integrated; page/file actions incomplete. | Not captured | Not captured | Unverified | Unverified | #7 |
| T07-03 | Edit menu: every item, focus-dependent enabled/disabled state. Default native menu; inventory pending. | Not captured | Not captured | Unverified | Unverified | #7 |
| T07-04 | View menu: every item, toolbar/tab commands. Partly integrated; additional actions incomplete. | Not captured | Not captured | Unverified | Unverified | #7 |
| T07-05 | History menu: every item and recently closed states. Partly integrated. | Not captured | Not captured | Unverified | Unverified | #6, #7 |
| T07-06 | Former sidebar Bookmarks menu and saved-page actions. Not planned per user; removed. | Not captured | Not captured | Not planned per user | Not planned per user | #5, #7 |
| T07-07 | Window and Help menus: every item and no-window state. Default menu plus window actions; inventory pending. | Not captured | Not captured | Unverified | Unverified | #7 |
| T07-08 | Page and editable-text context menus: every item/state. Native engine behavior; exact inventory unverified. | Not captured | Not captured | Unverified | Unverified | #7 |
| T07-09 | Find: empty query, matches, no match, next/previous, dismissal. Not in this bundle; separate implementation pending integration. | Not captured | Not captured | Unverified | Unverified | #7 |
| T07-10 | Page Zoom: in, out, actual size, disabled limits. Not in this bundle; separate implementation pending integration. | Not captured | Not captured | Unverified | Unverified | #7 |
| T07-11 | Reader: available, unavailable, options and dismissal. Not implemented. | Not captured | Not captured | Unverified | Unverified | #7 |
| T07-12 | Translation: available, unavailable, language/options dialogs. Not implemented. | Not captured | Not captured | Unverified | Unverified | #7 |
| T07-13 | Share: native picker, destinations, cancel and result. Not in this bundle; separate implementation pending integration. | Not captured | Not captured | Unverified | Unverified | #7 |
| T07-14 | Print: native sheet, options, preview, PDF submenus, cancel. Not in this bundle; separate implementation pending integration. | Not captured | Not captured | Unverified | Unverified | #7 |
| T07-15 | Save page/PDF: format, destination, extension and errors. Not implemented. | Not captured | Not captured | Unverified | Unverified | #7, #8 |
| T07-16 | Toolbar customization: context menu, palette, drag and restore. Not implemented as an application feature. | Not captured | Not captured | Unverified | Unverified | #7 |
| T08-01 | Downloads: empty list, active item, progress and completion. Not in this bundle; separate implementation in progress. | Not captured | Not captured | Unverified | Unverified | #8 |
| T08-02 | Download pause/resume, cancel, failed, retry, open and reveal. Not in this bundle; separate implementation in progress. | Not captured | Not captured | Unverified | Unverified | #8 |
| T08-03 | Download destination chooser: directory, filename and cancel. Not in this bundle; separate implementation in progress. | Not captured | Not captured | Unverified | Unverified | #8 |
| T08-04 | Download overwrite/replace and destination/write error dialogs. Not in this bundle; separate implementation in progress. | Not captured | Not captured | Unverified | Unverified | #8 |
| T08-05 | File upload chooser: one/multiple files, folders, cancel. Native engine callback path; UI unverified. | Not captured | Not captured | Unverified | Unverified | #8 |
| T08-06 | Open/import/export file workflows and error dialogs. Not implemented. | Not captured | Not captured | Unverified | Unverified | #8 |
| T09-01 | Settings: every pane, controls, default and changed values. Not implemented. | Not captured | Not captured | Unverified | Unverified | #9 |
| T09-02 | Website permissions: prompt, allow, deny, stored and reset. No complete application permission UI; unverified. | Not captured | Not captured | Unverified | Unverified | #9 |
| T09-03 | Website data: list, search, remove and confirmation. Not implemented. | Not captured | Not captured | Unverified | Unverified | #9 |
| T09-04 | Private browsing: new window, visual marker and close. Not implemented; persistence opt-out flag is not private mode. | Not captured | Not captured | Unverified | Unverified | #9 |
| T09-05 | Private history/storage/download separation and restart. Not implemented as a complete private session. | Not captured | Not captured | Unverified | Unverified | #9 |
| T09-06 | Website security: information popover, certificate and errors. Not implemented as an application feature. | Not captured | Not captured | Unverified | Unverified | #9 |
| T09-07 | Authentication: HTTP/proxy, username/password, retry/cancel. No complete application authentication dialogs; unverified. | Not captured | Not captured | Unverified | Unverified | #9 |
| T09-08 | Credentials/passkeys: native prompt, choice, cancel and errors. Not implemented or verified end to end. | Not captured | Not captured | Unverified | Unverified | #9 |
| T09-09 | Web content alert, confirm and prompt: every button/result. Native delegate support not confirmed; unverified. | Not captured | Not captured | Unverified | Unverified | #9, #10 |
| T09-10 | Before-unload, external protocol and invalid navigation dialogs. No complete application dialog inventory; unverified. | Not captured | Not captured | Unverified | Unverified | #9, #10 |
| T09-11 | Permission/media capture indicators and revoked permission. Not implemented or verified end to end. | Not captured | Not captured | Unverified | Unverified | #9 |
| T10-01 | Light/dark appearance for every UI row and native sheet. No complete paired coverage. | Not captured | Not captured | Unverified | Unverified | #10 |
| T10-02 | Focus/hover/pressed/disabled/keyboard states for every control. No complete paired coverage. | Not captured | Not captured | Unverified | Unverified | #10 |
| T10-03 | Open/close/resize/reorder/popup and dialog transition recordings. No complete paired motion coverage. | Not captured | Not captured | Unverified | Unverified | #10 |
| T10-04 | Reduced motion, accessibility labels and keyboard-only flow. No complete paired coverage. | Not captured | Not captured | Unverified | Unverified | #10 |

The existing navigation, tab, start-page and relationship verification notes
contain narrower source/runtime results. They do not replace this paired
comparison matrix. In particular, the standalone popup relationship fixture
passed scripting checks before integration; the integrated UI and motion still
require fresh comparison. No menu, alert, dialog or sheet is exempt because it
is supplied by the operating system or the web engine.
