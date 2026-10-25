# Start page and address suggestions

Scope: issue #4. The local start page appears for blank tabs and windows. Its search field opens the owning remote tab. Favorites use the saved-page library from issue #5, and Recently Visited uses this application's history store from issue #6. No browsing records are imported from other applications. Open Tabs excludes windows marked as nonpersistent; this is also the contract for address-tab candidates.

The start page supports adding favorites, switching to existing open tabs, hiding Favorites or Recently Visited, and an optional generated color background. Section and background settings persist in the application data directory. Website tiles use text initials; they do not load icons from external servers.

The native address field displays matching open tabs, saved bookmarks and recent destinations. An entered query is also offered as a search or address action. Up/Down selects a row, Return opens it, and Escape first dismisses the suggestions, then restores a changed address, then returns focus to content. Editing completion, tab changes, window deactivation and address-width changes dismiss the panel. Candidate rows are limited to the available window height.

The blank remote child is hidden while the local start page is shown. Navigation from local surfaces requests a one-time focus transfer after the destination child becomes visible; a tab switch cancels that request. Open Tabs refreshes on tab creation, closure, title updates and window destruction. Its snapshot requests discard outdated responses and refresh again after event listeners are ready. The same visibility calculation respects the history surface from issue #6. Sidebar UI is not planned per user; the start and history surfaces use the full content width.

Local source validation: Biome formatting, check and lint; TypeScript production build; rustfmt; Clippy with all targets and warnings denied; nine Rust tests; debug application bundling. The Rust tests cover address authorization/resolution, delayed one-time destination focus and the shared saved-page/history stores. They do not exercise the start page or AppKit suggestion controls.

Native interaction, appearance, focus, small-window layout, light/dark appearance and motion comparison remain pending. This implementation does not establish visual or motion equivalence. Reference captures include personal toolbar contents and must remain local.
