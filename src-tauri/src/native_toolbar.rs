use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::*;
use objc2_foundation::{
    MainThreadMarker, NSArray, NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSet,
    NSSize, NSString, ns_string,
};
use tauri::Manager;

use crate::NavigationState;

thread_local! {
    static TOOLBAR: RefCell<Option<Retained<BrowserToolbar>>> = const { RefCell::new(None) };
}

struct AddressState {
    url: RefCell<String>,
}

// ---------------------------------------------------------
// Expand the address before AppKit starts accepting input.
// Editing notifications arrive after the first key press.
// ---------------------------------------------------------
define_class!(
    #[unsafe(super = NSTextField)]
    #[thread_kind = MainThreadOnly]
    #[ivars = AddressState]
    struct AddressField;

    unsafe impl NSObjectProtocol for AddressField {}

    impl AddressField {
        #[unsafe(method(focusRingMaskBounds))]
        fn focus_mask_bounds(&self) -> NSRect {
            let frame = self.frame();
            let size = unsafe { self.superview() }.map(|view| view.bounds().size).unwrap_or(frame.size);
            NSRect::new(NSPoint::new(-frame.origin.x, -frame.origin.y), size)
        }

        #[unsafe(method(drawFocusRingMask))]
        fn draw_focus_mask(&self) {
            let frame = self.frame();
            let size = unsafe { self.superview() }.map(|view| view.bounds().size).unwrap_or(frame.size);
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                NSRect::new(NSPoint::new(-frame.origin.x, -frame.origin.y), size), 18.0, 18.0).fill();
        }

        #[unsafe(method(becomeFirstResponder))]
        fn become_first_responder(&self) -> bool {
            self.setAlignment(NSTextAlignment::Left);
            if self.currentEditor().is_none() {
                self.setStringValue(&NSString::from_str(&self.ivars().url.borrow()));
            }
            let accepted: bool = unsafe { msg_send![super(self), becomeFirstResponder] };
            accepted
        }
    }
);

struct ToolbarControls {
    window: Retained<NSWindow>,
    handle: tauri::AppHandle,
    history: Retained<NSSegmentedControl>,
    address: Retained<AddressField>,
    reload: Retained<NSButton>,
    progress: Retained<NSProgressIndicator>,
    history_item: Retained<NSToolbarItem>,
    address_item: Retained<NSToolbarItem>,
    address_width: Retained<NSLayoutConstraint>,
    loading: Cell<Option<bool>>,
    tabs: Retained<NSSegmentedControl>,
    tab_labels: RefCell<Vec<String>>,
}

// ---------------------------------------------------------
// AppKit owns the toolbar's layout, materials and interaction.
// Keep its weak delegate and action targets alive on main.
// ---------------------------------------------------------
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ToolbarControls]
    struct BrowserToolbar;

    unsafe impl NSObjectProtocol for BrowserToolbar {}

    unsafe impl NSToolbarDelegate for BrowserToolbar {
        #[unsafe(method_id(toolbarDefaultItemIdentifiers:))]
        fn default_items(&self, _toolbar: &NSToolbar) -> Retained<NSArray<NSString>> {
            unsafe { NSArray::from_slice(&[ns_string!("history"), NSToolbarFlexibleSpaceItemIdentifier,
                ns_string!("address"), NSToolbarFlexibleSpaceItemIdentifier]) }
        }

        #[unsafe(method_id(toolbarAllowedItemIdentifiers:))]
        fn allowed_items(&self, toolbar: &NSToolbar) -> Retained<NSArray<NSString>> {
            let _ = toolbar;
            unsafe { NSArray::from_slice(&[ns_string!("history"), NSToolbarFlexibleSpaceItemIdentifier, ns_string!("address"), NSToolbarFlexibleSpaceItemIdentifier]) }
        }

        #[unsafe(method_id(toolbar:itemForItemIdentifier:willBeInsertedIntoToolbar:))]
        fn item(&self, _toolbar: &NSToolbar, identifier: &NSString, _inserted: bool) -> Option<Retained<NSToolbarItem>> {
            match identifier.to_string().as_str() {
                "history" => Some(self.ivars().history_item.clone()),
                "address" => Some(self.ivars().address_item.clone()),
                _ => None,
            }
        }
    }

    unsafe impl NSControlTextEditingDelegate for BrowserToolbar {
        #[unsafe(method(controlTextDidEndEditing:))]
        fn end_editing(&self, _notification: &NSNotification) {
            self.ivars().address.setAlignment(NSTextAlignment::Center);
        }

        #[unsafe(method(control:textView:doCommandBySelector:))]
        fn command(&self, _control: &NSControl, _editor: &NSTextView, command: objc2::runtime::Sel) -> bool {
            if command != sel!(cancelOperation:) { return false.into(); }
            let controls = self.ivars();
            let url = NSString::from_str(&controls.address.ivars().url.borrow());
            if controls.address.stringValue() != url {
                controls.address.setStringValue(&url);
                _editor.setString(&url);
                unsafe { _editor.selectAll(None); }
            } else {
                controls.window.makeFirstResponder(None);
                if let Some(content) = crate::tabs::active(&controls.handle) {
                    let _ = content.set_focus();
                }
            }
            true
        }
    }

    unsafe impl NSTextFieldDelegate for BrowserToolbar {}

    impl BrowserToolbar {
        #[unsafe(method(navigate:))]
        fn navigate(&self, _sender: &NSTextField) {
            let controls = self.ivars();
            let address = controls.address.stringValue().to_string();
            let result = crate::resolve_address(&address).and_then(|url| {
                crate::tabs::active(&controls.handle).ok_or_else(|| String::from("The browser WebView is not ready."))?
                    .navigate(url).map_err(|error| error.to_string())
            });
            if let Err(error) = result {
                let alert = NSAlert::new(self.mtm());
                alert.setMessageText(ns_string!("Could not open this address"));
                alert.setInformativeText(&NSString::from_str(&error));
                alert.runModal();
                return;
            }
            if let Some(window) = controls.address.window() { window.makeFirstResponder(None); }
            if let Some(content) = crate::tabs::active(&controls.handle) { let _ = content.set_focus(); }
        }

        #[unsafe(method(navigateHistory:))]
        fn navigate_history(&self, sender: &NSSegmentedControl) {
            perform(if sender.selectedSegment() == 0 { "back" } else { "forward" });
        }

        #[unsafe(method(selectTab:))]
        fn select_tab(&self, sender: &NSSegmentedControl) {
            let label = self.ivars().tab_labels.borrow().get(sender.selectedSegment() as usize).cloned();
            if let Some(label) = label
                && let Err(error) = crate::tabs::select(&self.ivars().handle, &label) { eprintln!("Could not select tab: {error}"); }
        }

        #[unsafe(method(newTab:))]
        fn new_tab(&self, _sender: &NSButton) {
            if let Err(error) = crate::tabs::perform(&self.ivars().handle, "new-tab") { eprintln!("Could not create tab: {error}"); }
        }

        #[unsafe(method(closeTab:))]
        fn close_tab(&self, _sender: &NSButton) {
            let handle = self.ivars().handle.clone();
            if let Err(error) = crate::tabs::perform(&handle, "close-tab") { eprintln!("Could not close tab: {error}"); }
        }

        #[unsafe(method(reloadOrStop:))]
        fn reload_or_stop(&self, _sender: &NSButton) {
            perform("reload-or-stop");
        }
    }
);

// ---------------------------------------------------------
// Attach native controls without replacing Tauri's delegate.
// Fixed height and flexible width survive window resizing.
// ---------------------------------------------------------
pub fn install(window: &tauri::Window) -> Result<(), Box<dyn std::error::Error>> {
    let mtm = MainThreadMarker::new().ok_or("Toolbar setup must run on the main thread.")?;
    let native = unsafe { Retained::retain(window.ns_window()?.cast::<NSWindow>()) }
        .ok_or("The native window is not ready.")?;
    let history = NSSegmentedControl::new(mtm);
    history.setControlSize(NSControlSize::Large);
    history.setSegmentCount(2);
    history.setTrackingMode(NSSegmentSwitchTracking::Momentary);
    for (index, symbol, label) in [
        (0, "chevron.backward", "Back"),
        (1, "chevron.forward", "Forward"),
    ] {
        history.setImage_forSegment(
            NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &NSString::from_str(symbol),
                Some(&NSString::from_str(label)),
            )
            .as_deref(),
            index,
        );
        history.setToolTip_forSegment(Some(&NSString::from_str(label)), index);
        history.setWidth_forSegment(32.0, index);
        history.setEnabled_forSegment(false, index);
    }
    history.sizeToFit();
    let history_item =
        NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), ns_string!("history"));
    history_item.setView(Some(&history));
    history_item.setNavigational(true);
    history_item.setAutovalidates(false);
    history_item.setLabel(ns_string!("Back and Forward"));

    let glass = NSGlassEffectView::initWithFrame(
        NSGlassEffectView::alloc(mtm),
        NSRect::new(NSPoint::ZERO, NSSize::new(484.0, 36.0)),
    );
    glass.setCornerRadius(18.0);
    let container = NSView::initWithFrame(NSView::alloc(mtm), glass.bounds());
    let address = AddressField::alloc(mtm).set_ivars(AddressState {
        url: RefCell::new(String::new()),
    });
    let address: Retained<AddressField> = unsafe {
        msg_send![super(address), initWithFrame: NSRect::new(NSPoint::new(30.0, 8.0), NSSize::new(414.0, 21.0))]
    };
    address.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    address.setEditable(true);
    address.setSelectable(true);
    address.setEnabled(true);
    address.setBordered(false);
    address.setBezeled(false);
    address.setDrawsBackground(false);
    address.setFont(Some(&NSFont::systemFontOfSize(13.0)));
    address.setAlignment(NSTextAlignment::Center);
    address.setPlaceholderString(Some(ns_string!("Search or enter website name")));
    address.setAccessibilityLabel(Some(ns_string!("Search or enter a URL")));
    let reload = NSButton::new(mtm);
    reload.setFrame(NSRect::new(
        NSPoint::new(450.0, 4.0),
        NSSize::new(28.0, 28.0),
    ));
    reload.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
    reload.setBordered(false);
    reload.setTitle(ns_string!(""));
    reload.setImage(
        NSImage::imageWithSystemSymbolName_accessibilityDescription(
            ns_string!("arrow.clockwise"),
            Some(ns_string!("Reload this page")),
        )
        .as_deref(),
    );
    reload.setAccessibilityLabel(Some(ns_string!("Reload this page")));
    let progress_clip = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(12.0, 0.0), NSSize::new(460.0, 1.0)),
    );
    progress_clip.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    progress_clip.setWantsLayer(true);
    if let Some(layer) = progress_clip.layer() {
        layer.setMasksToBounds(true);
    }
    let progress = NSProgressIndicator::initWithFrame(
        NSProgressIndicator::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, -1.0), NSSize::new(460.0, 4.0)),
    );
    progress.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    progress.setControlSize(NSControlSize::Mini);
    progress.setIndeterminate(false);
    progress.setMinValue(0.0);
    progress.setMaxValue(1.0);
    progress.setHidden(true);
    container.addSubview(&address);
    container.addSubview(&reload);
    progress_clip.addSubview(&progress);
    container.addSubview(&progress_clip);
    glass.setContentView(Some(&container));
    glass.setTranslatesAutoresizingMaskIntoConstraints(false);
    let address_width = glass.widthAnchor().constraintEqualToConstant(484.0);
    address_width.setActive(true);
    glass
        .heightAnchor()
        .constraintEqualToConstant(36.0)
        .setActive(true);
    let address_item =
        NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), ns_string!("address"));
    address_item.setView(Some(&glass));
    address_item.setBordered(false);
    address_item.setLabel(ns_string!("Address and Search"));
    address_item.setAutovalidates(false);
    address_item.setEnabled(true);
    let tabs = NSSegmentedControl::initWithFrame(
        NSSegmentedControl::alloc(mtm),
        NSRect::new(NSPoint::new(8.0, 4.0), NSSize::new(1008.0, 30.0)),
    );
    tabs.setTrackingMode(NSSegmentSwitchTracking::SelectOne);
    tabs.setSegmentStyle(NSSegmentStyle::TexturedRounded);
    tabs.setSelectedSegmentBezelColor(Some(&NSColor::controlBackgroundColor()));
    tabs.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    tabs.setAccessibilityLabel(Some(ns_string!("Tabs")));
    let strip = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::ZERO, NSSize::new(1100.0, 38.0)),
    );
    strip.addSubview(&tabs);
    let new_tab = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(1058.0, 4.0), NSSize::new(32.0, 30.0)),
    );
    new_tab.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
    new_tab.setBordered(false);
    new_tab.setImage(
        NSImage::imageWithSystemSymbolName_accessibilityDescription(
            ns_string!("plus"),
            Some(ns_string!("New Tab")),
        )
        .as_deref(),
    );
    new_tab.setAccessibilityLabel(Some(ns_string!("New Tab")));
    strip.addSubview(&new_tab);
    let close_tab = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(1022.0, 4.0), NSSize::new(32.0, 30.0)),
    );
    close_tab.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
    close_tab.setBordered(false);
    close_tab.setImage(
        NSImage::imageWithSystemSymbolName_accessibilityDescription(
            ns_string!("xmark"),
            Some(ns_string!("Close Tab")),
        )
        .as_deref(),
    );
    close_tab.setAccessibilityLabel(Some(ns_string!("Close Tab")));
    strip.addSubview(&close_tab);
    let accessory = NSTitlebarAccessoryViewController::new(mtm);
    accessory.setView(&strip);
    accessory.setLayoutAttribute(NSLayoutAttribute::Bottom);
    accessory.setFullScreenMinHeight(38.0);
    native.addTitlebarAccessoryViewController(&accessory);
    let delegate = BrowserToolbar::alloc(mtm).set_ivars(ToolbarControls {
        window: native.clone(),
        handle: window.app_handle().clone(),
        history,
        address,
        reload,
        progress,
        history_item,
        address_item,
        address_width,
        loading: Cell::new(None),
        tabs,
        tab_labels: RefCell::new(Vec::new()),
    });
    let delegate: Retained<BrowserToolbar> = unsafe { msg_send![super(delegate), init] };
    let controls = delegate.ivars();
    unsafe {
        controls.tabs.setTarget(Some(&delegate));
        controls.tabs.setAction(Some(sel!(selectTab:)));
        new_tab.setTarget(Some(&delegate));
        new_tab.setAction(Some(sel!(newTab:)));
        close_tab.setTarget(Some(&delegate));
        close_tab.setAction(Some(sel!(closeTab:)));
        controls.address.setTarget(Some(&delegate));
        controls.address.setAction(Some(sel!(navigate:)));
        controls
            .address
            .setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        controls.history.setTarget(Some(&delegate));
        controls.history.setAction(Some(sel!(navigateHistory:)));
        controls.reload.setTarget(Some(&delegate));
        controls.reload.setAction(Some(sel!(reloadOrStop:)));
    }
    let toolbar =
        NSToolbar::initWithIdentifier(NSToolbar::alloc(mtm), ns_string!("BrowserToolbar"));
    toolbar.setDisplayMode(NSToolbarDisplayMode::IconOnly);
    toolbar.setCenteredItemIdentifiers(&NSSet::from_slice(&[ns_string!("address")]));
    toolbar.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    native.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    native.setToolbarStyle(NSWindowToolbarStyle::Unified);
    native.setTitlebarSeparatorStyle(NSTitlebarSeparatorStyle::None);
    native.setToolbar(Some(&toolbar));
    toolbar.setVisible(true);
    TOOLBAR.with(|slot| {
        *slot.borrow_mut() = Some(delegate);
    });
    Ok(())
}

// ---------------------------------------------------------
// Synchronize controls without changing an active text edit.
// Native properties also cover fragment and failed loads.
// ---------------------------------------------------------
pub fn update(state: &NavigationState) {
    TOOLBAR.with(|slot| {
        let borrowed = slot.borrow();
        let Some(toolbar) = borrowed.as_ref() else {
            return;
        };
        let controls = toolbar.ivars();
        controls.history.setEnabled_forSegment(state.can_go_back, 0);
        controls
            .history
            .setEnabled_forSegment(state.can_go_forward, 1);
        let address = if state.url == "about:blank" {
            String::new()
        } else {
            state.url.clone()
        };
        *controls.address.ivars().url.borrow_mut() = address.clone();
        if controls.address.currentEditor().is_none() {
            if controls.address.alignment() != NSTextAlignment::Center {
                controls.address.setAlignment(NSTextAlignment::Center);
            }
            let display = tauri::Url::parse(&state.url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .unwrap_or(address);
            controls
                .address
                .setStringValue(&NSString::from_str(&display));
        }
        if controls.loading.replace(Some(state.loading)) != Some(state.loading) {
            if let Some(item) = controls
                .handle
                .menu()
                .and_then(|menu| menu.get("stop-page"))
                .and_then(|item| item.as_menuitem().cloned())
            {
                let _ = item.set_enabled(state.loading);
            }
            let (symbol, label) = if state.loading {
                ("xmark", "Stop loading")
            } else {
                ("arrow.clockwise", "Reload this page")
            };
            controls.reload.setImage(
                NSImage::imageWithSystemSymbolName_accessibilityDescription(
                    &NSString::from_str(symbol),
                    Some(&NSString::from_str(label)),
                )
                .as_deref(),
            );
            controls
                .reload
                .setAccessibilityLabel(Some(&NSString::from_str(label)));
            controls.reload.setToolTip(Some(&NSString::from_str(label)));
        }
        controls.progress.setHidden(!state.loading);
        controls.progress.setDoubleValue(state.progress);
        // ---------------------------------------------------------
        // AppKit can detach the field into toolbar overflow.
        // Read the owner window so it can return after resizing.
        // ---------------------------------------------------------
        let width = (controls.window.frame().size.width * 0.41).max(220.0);
        if (controls.address_width.constant() - width).abs() > 0.5 {
            controls.address_width.setConstant(width);
        }
    });
}

pub fn remove() {
    TOOLBAR.with(|slot| {
        slot.borrow_mut().take();
    });
}

// ---------------------------------------------------------
// Handle shortcuts even when web content owns keyboard focus.
// ---------------------------------------------------------
pub fn perform(action: &str) {
    TOOLBAR.with(|slot| {
        let borrowed = slot.borrow();
        let Some(toolbar) = borrowed.as_ref() else {
            return;
        };
        let controls = toolbar.ivars();
        if action == "focus-address" {
            if let Some(editor) = controls.address.currentEditor() {
                unsafe {
                    editor.selectAll(None);
                }
                return;
            }
            controls.address.setAlignment(NSTextAlignment::Left);
            if let Some(window) = controls.address.window() {
                window.makeFirstResponder(Some(&controls.address));
            }
            unsafe {
                controls.address.selectText(None);
            }
            return;
        }
        let Some(content) = crate::tabs::active(&controls.handle) else {
            return;
        };
        let action = action.to_string();
        let _ = content.with_webview(move |platform| unsafe {
            let native = &*platform.inner().cast::<objc2_web_kit::WKWebView>();
            match action.as_str() {
                "back" if native.canGoBack() => {
                    native.goBack();
                }
                "forward" if native.canGoForward() => {
                    native.goForward();
                }
                "reload-or-stop" if native.isLoading() => native.stopLoading(),
                "reload-or-stop" | "reload-page" => {
                    native.reload();
                }
                "stop-page" => native.stopLoading(),
                _ => {}
            }
        });
    });
}

// ---------------------------------------------------------
// End the old tab's edit before displaying the new address.
// The next native poll supplies its history and load state.
// ---------------------------------------------------------
pub fn tab_changed(app: &tauri::AppHandle) {
    TOOLBAR.with(|slot| {
        let borrowed = slot.borrow();
        let Some(toolbar) = borrowed.as_ref() else {
            return;
        };
        let controls = toolbar.ivars();
        controls.window.makeFirstResponder(None);
        let url = crate::tabs::active(app)
            .and_then(|content| crate::tabs::current_url(&content).ok())
            .map(|url| url.to_string())
            .unwrap_or_default();
        let address = if url == "about:blank" {
            String::new()
        } else {
            url
        };
        *controls.address.ivars().url.borrow_mut() = address.clone();
        controls
            .address
            .setStringValue(&NSString::from_str(&address));
        controls.history.setEnabled_forSegment(false, 0);
        controls.history.setEnabled_forSegment(false, 1);
        controls.progress.setHidden(true);
    });
    update_tabs(app);
}

pub fn update_tabs(app: &tauri::AppHandle) {
    let (entries, active) = {
        let state = app.state::<std::sync::Mutex<crate::tabs::Tabs>>();
        let Ok(tabs) = state.lock() else {
            return;
        };
        (tabs.entries.clone(), tabs.active.clone())
    };
    TOOLBAR.with(|slot| {
        let borrowed = slot.borrow();
        let Some(toolbar) = borrowed.as_ref() else {
            return;
        };
        let controls = toolbar.ivars();
        controls.tabs.setSegmentCount(entries.len() as isize);
        let width =
            ((controls.window.frame().size.width - 92.0) / entries.len().max(1) as f64).max(40.0);
        for (index, tab) in entries.iter().enumerate() {
            let title = if tab.title.is_empty() {
                "New Tab"
            } else {
                &tab.title
            };
            controls
                .tabs
                .setLabel_forSegment(&NSString::from_str(title), index as isize);
            controls.tabs.setWidth_forSegment(width, index as isize);
            controls
                .tabs
                .setSelected_forSegment(tab.label == active, index as isize);
            controls
                .tabs
                .setToolTip_forSegment(Some(&NSString::from_str(title)), index as isize);
        }
        *controls.tab_labels.borrow_mut() = entries.into_iter().map(|tab| tab.label).collect();
    });
}
