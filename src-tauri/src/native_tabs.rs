use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::*;
use objc2_foundation::{
    MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, ns_string,
};
use tauri::Manager;

use crate::tabs::{Tab, Windows};

type StripSnapshot = (Vec<Tab>, String, u64);

struct TabStrip {
    container: Retained<NSView>,
    pinned_scroll: Retained<TabScrollView>,
    pinned_document: Retained<NSView>,
    scroll: Retained<TabScrollView>,
    document: Retained<NSView>,
    snapshot: Option<StripSnapshot>,
}

thread_local! {
    static STRIP: RefCell<HashMap<String, TabStrip>> = RefCell::new(HashMap::new());
}

// ---------------------------------------------------------
// Keep system input-device changes from reserving scrollbar
// space inside the fixed-height tab accessory.
// ---------------------------------------------------------
define_class!(
    #[unsafe(super = NSScrollView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    struct TabScrollView;

    unsafe impl NSObjectProtocol for TabScrollView {}

    impl TabScrollView {
        #[unsafe(method(setScrollerStyle:))]
        fn set_scroller_style(&self, _style: NSScrollerStyle) {
            unsafe { let _: () = msg_send![super(self), setScrollerStyle: NSScrollerStyle::Overlay]; }
        }
    }
);

#[derive(Clone)]
struct TabAction {
    handle: tauri::AppHandle,
    label: String,
    window_label: String,
    close: bool,
    pinned: bool,
}

// ---------------------------------------------------------
// Bind controls to stable labels rather than mutable indices.
// Clone action data before layout can release this button.
// ---------------------------------------------------------
define_class!(
    #[unsafe(super = NSButton)]
    #[thread_kind = MainThreadOnly]
    #[ivars = TabAction]
    struct TabButton;

    unsafe impl NSObjectProtocol for TabButton {}

    impl TabButton {
        #[unsafe(method_id(menuForEvent:))]
        fn context_menu(&self, _event: &NSEvent) -> Option<Retained<NSMenu>> {
            tab_menu(self)
        }

        #[unsafe(method(performTabMenu:))]
        fn menu_action(&self, sender: &NSMenuItem) {
            let action = self.ivars().clone();
            let metadata = action.handle.state::<Mutex<Windows>>().lock().ok()
                .and_then(|windows| windows.entries.get(&action.window_label)
                    .and_then(|tabs| tabs.entries.iter().enumerate().find(|(_, tab)| tab.label == action.label))
                    .map(|(index, tab)| (index, tab.pinned)));
            let Some((index, pinned)) = metadata else { return; };
            let result = match sender.tag() {
                0 => crate::tabs::set_pinned(&action.handle, &action.window_label, &action.label, !pinned),
                1 => crate::tabs::reorder(&action.handle, &action.window_label, &action.label, index.saturating_sub(1)),
                2 => crate::tabs::reorder(&action.handle, &action.window_label, &action.label, index + 1),
                3 => crate::tabs::close_tab(&action.handle, &action.window_label, &action.label),
                _ => return,
            };
            if let Err(error) = result { eprintln!("Tab menu action failed: {error}"); }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if self.ivars().close {
                unsafe { let _: () = msg_send![super(self), mouseDown: event]; }
                return;
            }
            if event.modifierFlags().contains(NSEventModifierFlags::Control) {
                if let Some(menu) = tab_menu(self) {
                    NSMenu::popUpContextMenu_withEvent_forView(&menu, event, self);
                }
                return;
            }
            // ---------------------------------------------------------
            // Retain the sender across layout changes after tracking.
            // Move its cell during dragging; commit by stable label.
            // ---------------------------------------------------------
            let _retained = unsafe { Retained::retain(self as *const Self as *mut Self) };
            let Some(window) = self.window() else { return; };
            let Some(cell) = (unsafe { self.superview() }) else { return; };
            let Some(document) = (unsafe { cell.superview() }) else { return; };
            let origin = cell.frame().origin;
            let start = event.locationInWindow();
            let start_document = document.convertPoint_fromView(start, None);
            let mut dragged = false;
            let mut destination = None;
            while let Some(next) = window.nextEventMatchingMask(NSEventMask::LeftMouseDragged | NSEventMask::LeftMouseUp) {
                let point = document.convertPoint_fromView(next.locationInWindow(), None);
                let distance = next.locationInWindow().x - start.x;
                dragged |= distance.abs() > 4.0;
                if dragged {
                    cell.setFrameOrigin(NSPoint::new((origin.x + point.x - start_document.x).max(0.0), origin.y));
                    document.scrollRectToVisible(NSRect::new(NSPoint::new(point.x - 20.0, 0.0), NSSize::new(40.0, 32.0)));
                    destination = drop_position(&self.ivars().window_label, self.ivars().pinned, point.x);
                }
                if next.r#type() == NSEventType::LeftMouseUp { break; }
            }
            cell.setFrameOrigin(origin);
            let action = self.ivars().clone();
            let result = if let Some(position) = destination {
                crate::tabs::reorder(&action.handle, &action.window_label, &action.label, position)
            } else {
                crate::tabs::select(&action.handle, &action.window_label, &action.label)
            };
            if let Err(error) = result { eprintln!("Tab tracking failed: {error}"); }
        }

        #[unsafe(method(activateTab:))]
        fn activate(&self, _sender: &NSButton) {
            let handle = self.ivars().handle.clone();
            let label = self.ivars().label.clone();
            let window = self.ivars().window_label.clone();
            let result = if self.ivars().close {
                crate::tabs::close_tab(&handle, &window, &label)
            } else {
                crate::tabs::select(&handle, &window, &label)
            };
            if let Err(error) = result { eprintln!("Tab action failed: {error}"); }
        }
    }
);

fn tab_menu(button: &TabButton) -> Option<Retained<NSMenu>> {
    if button.ivars().close {
        return None;
    }
    let action = button.ivars();
    let state = action.handle.state::<Mutex<Windows>>();
    let windows = state.lock().ok()?;
    let tabs = windows.entries.get(&action.window_label)?;
    let index = tabs
        .entries
        .iter()
        .position(|tab| tab.label == action.label)?;
    let tab = &tabs.entries[index];
    let first = tabs.entries.iter().take_while(|entry| entry.pinned).count();
    let menu = NSMenu::new(button.mtm());
    menu.setAutoenablesItems(false);
    let pin_title = if tab.pinned { "Unpin Tab" } else { "Pin Tab" };
    let minimum = if tab.pinned { 0 } else { first };
    let maximum = if tab.pinned {
        first - 1
    } else {
        tabs.entries.len() - 1
    };
    for (title, tag, enabled) in [
        (pin_title, 0, true),
        ("Move Tab Left", 1, index > minimum),
        ("Move Tab Right", 2, index < maximum),
        ("Close Tab", 3, true),
    ] {
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(button.mtm()),
                &NSString::from_str(title),
                Some(sel!(performTabMenu:)),
                ns_string!(""),
            )
        };
        item.setTag(tag);
        item.setEnabled(enabled);
        unsafe {
            item.setTarget(Some(button));
            item.setRepresentedObject(Some(button));
        }
        menu.addItem(&item);
    }
    Some(menu)
}

// ---------------------------------------------------------
// Share compact pinned geometry between layout and dropping.
// Reordering is clamped to the original pinned group in Tabs.
// ---------------------------------------------------------
fn tab_widths(entries: &[Tab], available: f64) -> (usize, f64) {
    let pinned = entries.iter().take_while(|tab| tab.pinned).count();
    let normal = entries.len() - pinned;
    let shelf = (pinned as f64 * 42.0).min(if normal == 0 {
        available
    } else {
        available / 2.0
    });
    let width = ((available - shelf) / normal.max(1) as f64).max(120.0);
    (pinned, width)
}

fn drop_position(window: &str, is_pinned: bool, x: f64) -> Option<usize> {
    STRIP.with(|slot| {
        let borrowed = slot.borrow();
        let strip = borrowed.get(window)?;
        let (entries, _, available) = strip.snapshot.as_ref()?;
        let (pinned, width) = tab_widths(entries, *available as f64);
        let position = if is_pinned {
            (x.max(0.0) / 42.0) as usize
        } else {
            pinned + (x.max(0.0) / width) as usize
        };
        Some(position.min(entries.len().saturating_sub(1)))
    })
}

fn button(
    action: TabAction,
    title: &str,
    frame: NSRect,
    selected: bool,
    mtm: MainThreadMarker,
) -> Retained<TabButton> {
    let close = action.close;
    let button = TabButton::alloc(mtm).set_ivars(action);
    let button: Retained<TabButton> = unsafe { msg_send![super(button), initWithFrame: frame] };
    button.setBordered(false);
    button.setFont(Some(&NSFont::systemFontOfSize(13.0)));
    button.setTitle(&NSString::from_str(if close { "" } else { title }));
    button.setAccessibilityLabel(Some(&NSString::from_str(&if close {
        format!("Close {title}")
    } else {
        title.into()
    })));
    if close {
        button.setImage(
            NSImage::imageWithSystemSymbolName_accessibilityDescription(
                ns_string!("xmark"),
                Some(ns_string!("Close Tab")),
            )
            .as_deref(),
        );
    } else {
        button.setAccessibilityRole(Some(ns_string!("AXRadioButton")));
        button.setAccessibilitySelected(selected);
        if let Some(cell) = button.cell() {
            cell.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        }
    }
    unsafe {
        button.setTarget(Some(&button));
        button.setAction(Some(sel!(activateTab:)));
    }
    button
}

// ---------------------------------------------------------
// Let AppKit size the accessory and clip overflowing tabs.
// The document retains off-screen controls and stable actions.
// ---------------------------------------------------------
// ---------------------------------------------------------
// Keep pinned and regular overflow independent, so a selected
// regular tab does not scroll pinned controls off the left.
// ---------------------------------------------------------
fn scroll_view(
    frame: NSRect,
    mtm: MainThreadMarker,
) -> (Retained<TabScrollView>, Retained<NSView>) {
    let scroll = TabScrollView::alloc(mtm).set_ivars(());
    let scroll: Retained<TabScrollView> = unsafe { msg_send![super(scroll), initWithFrame: frame] };
    scroll.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    scroll.setBorderType(NSBorderType::NoBorder);
    scroll.setDrawsBackground(false);
    scroll.contentView().setDrawsBackground(false);
    scroll.setScrollerStyle(NSScrollerStyle::Overlay);
    scroll.setHasHorizontalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setHasVerticalScroller(false);
    let document = NSView::initWithFrame(NSView::alloc(mtm), scroll.bounds());
    document.setAccessibilityRole(Some(ns_string!("AXTabGroup")));
    document.setAccessibilityLabel(Some(ns_string!("Tabs")));
    scroll.setDocumentView(Some(&document));
    (scroll, document)
}

pub fn install(window: &tauri::Window) -> Result<(), Box<dyn std::error::Error>> {
    let mtm = MainThreadMarker::new().ok_or("Tab setup must run on the main thread.")?;
    let native = unsafe { &*window.ns_window()?.cast::<NSWindow>() };
    let width = native.frame().size.width;
    let root = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::ZERO, NSSize::new(width, 38.0)),
    );
    let glass = NSGlassEffectView::initWithFrame(
        NSGlassEffectView::alloc(mtm),
        NSRect::new(NSPoint::new(8.0, 3.0), NSSize::new(width - 16.0, 32.0)),
    );
    glass.setCornerRadius(16.0);
    glass.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    let frame = NSRect::new(NSPoint::ZERO, NSSize::new(width - 16.0, 32.0));
    let container = NSView::initWithFrame(NSView::alloc(mtm), frame);
    container.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    let (scroll, document) = scroll_view(frame, mtm);
    let (pinned_scroll, pinned_document) =
        scroll_view(NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 32.0)), mtm);
    pinned_scroll.setAutoresizingMask(NSAutoresizingMaskOptions::empty());
    pinned_document.setAccessibilityLabel(Some(ns_string!("Pinned Tabs")));
    container.addSubview(&scroll);
    container.addSubview(&pinned_scroll);
    glass.setContentView(Some(&container));
    root.addSubview(&glass);
    let accessory = NSTitlebarAccessoryViewController::new(mtm);
    accessory.setView(&root);
    accessory.setLayoutAttribute(NSLayoutAttribute::Bottom);
    accessory.setFullScreenMinHeight(38.0);
    native.addTitlebarAccessoryViewController(&accessory);
    STRIP.with(|slot| {
        slot.borrow_mut().insert(
            window.label().to_string(),
            TabStrip {
                container,
                pinned_scroll,
                pinned_document,
                scroll,
                document,
                snapshot: None,
            },
        );
    });
    Ok(())
}

pub fn update(app: &tauri::AppHandle, window_label: &str) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let (entries, active) = {
        let state = app.state::<Mutex<Windows>>();
        let Ok(windows) = state.lock() else {
            return;
        };
        let Some(tabs) = windows.entries.get(window_label) else {
            return;
        };
        (tabs.entries.clone(), tabs.active.clone())
    };
    STRIP.with(|slot| {
        let mut borrowed = slot.borrow_mut();
        let Some(strip) = borrowed.get_mut(window_label) else {
            return;
        };
        let available = strip.container.bounds().size.width.max(1.0);
        let snapshot = (entries.clone(), active.clone(), available.round() as u64);
        if strip.snapshot.as_ref() == Some(&snapshot) {
            return;
        }
        let changed_selection = strip.snapshot.as_ref().is_none_or(|previous| {
            previous.1 != active
                || previous.2 != snapshot.2
                || previous.0.len() != entries.len()
                || previous.0.iter().zip(&entries).any(|(before, after)| {
                    before.label != after.label || before.pinned != after.pinned
                })
        });
        strip.snapshot = Some(snapshot);
        for document in [&strip.document, &strip.pinned_document] {
            for view in document.subviews() {
                view.removeFromSuperview();
            }
        }
        let (pinned, normal_width) = tab_widths(&entries, available);
        let normal = entries.len() - pinned;
        let shelf = (pinned as f64 * 42.0).min(if normal == 0 {
            available
        } else {
            available / 2.0
        });
        strip
            .pinned_scroll
            .setFrame(NSRect::new(NSPoint::ZERO, NSSize::new(shelf, 32.0)));
        strip.scroll.setFrame(NSRect::new(
            NSPoint::new(shelf, 0.0),
            NSSize::new(available - shelf, 32.0),
        ));
        strip
            .pinned_document
            .setFrameSize(NSSize::new(pinned as f64 * 42.0, 32.0));
        strip.document.setFrameSize(NSSize::new(
            (normal_width * normal as f64).max(available - shelf),
            32.0,
        ));
        for (index, tab) in entries.iter().enumerate() {
            let width = if tab.pinned { 42.0 } else { normal_width };
            let x = if tab.pinned {
                index as f64 * 42.0
            } else {
                (index - pinned) as f64 * normal_width
            };
            let selected = tab.label == active;
            let title = if tab.title.is_empty() {
                "New Tab"
            } else {
                &tab.title
            };
            let frame = NSRect::new(NSPoint::new(x, 0.0), NSSize::new(width, 32.0));
            let cell = NSView::initWithFrame(NSView::alloc(mtm), frame);
            cell.setClipsToBounds(true);
            if selected {
                let glass = NSGlassEffectView::initWithFrame(
                    NSGlassEffectView::alloc(mtm),
                    NSRect::new(NSPoint::new(1.0, 1.0), NSSize::new(width - 2.0, 30.0)),
                );
                glass.setCornerRadius(15.0);
                cell.addSubview(&glass);
            }
            let action = TabAction {
                handle: app.clone(),
                label: tab.label.clone(),
                window_label: window_label.into(),
                close: false,
                pinned: tab.pinned,
            };
            let compact_title = title
                .chars()
                .next()
                .unwrap_or('•')
                .to_uppercase()
                .to_string();
            let label = button(
                action.clone(),
                if tab.pinned { &compact_title } else { title },
                NSRect::new(
                    NSPoint::new(if tab.pinned { 0.0 } else { 32.0 }, 0.0),
                    NSSize::new(if tab.pinned { width } else { width - 64.0 }, 32.0),
                ),
                selected,
                mtm,
            );
            label.setToolTip(Some(&NSString::from_str(title)));
            label.setAccessibilityLabel(Some(&NSString::from_str(&if tab.pinned {
                format!("Pinned tab: {title}")
            } else {
                title.into()
            })));
            cell.addSubview(&label);
            let close = button(
                TabAction {
                    close: true,
                    ..action
                },
                title,
                NSRect::new(NSPoint::new(4.0, 4.0), NSSize::new(24.0, 24.0)),
                false,
                mtm,
            );
            if !tab.pinned {
                cell.addSubview(&close);
            }
            let document = if tab.pinned {
                &strip.pinned_document
            } else {
                &strip.document
            };
            document.addSubview(&cell);
            if selected && changed_selection {
                document.scrollRectToVisible(frame);
            }
        }
    });
}

pub fn remove(window_label: &str) {
    crate::native_tab_navigation::remove_window(window_label);
    STRIP.with(|slot| {
        slot.borrow_mut().remove(window_label);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_overflow_keeps_room_for_regular_tabs() {
        let mut entries: Vec<Tab> = (0..20)
            .map(|index| Tab {
                label: index.to_string(),
                title: String::new(),
                pinned: true,
                pinned_url: None,
                url: String::new(),
                interaction: Vec::new(),
            })
            .collect();
        entries.push(Tab {
            label: "regular".into(),
            title: String::new(),
            pinned: false,
            pinned_url: None,
            url: String::new(),
            interaction: Vec::new(),
        });
        assert_eq!(tab_widths(&entries, 420.0), (20, 210.0));
        assert_eq!(tab_widths(&entries, 200.0), (20, 120.0));
    }
}
