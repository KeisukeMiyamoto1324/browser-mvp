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
    let scroll = TabScrollView::alloc(mtm).set_ivars(());
    let frame = NSRect::new(NSPoint::ZERO, NSSize::new(width - 16.0, 32.0));
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
    glass.setContentView(Some(&scroll));
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
        let available = strip.scroll.contentSize().width.max(1.0);
        let snapshot = (entries.clone(), active.clone(), available.round() as u64);
        if strip.snapshot.as_ref() == Some(&snapshot) {
            return;
        }
        let changed_selection = strip.snapshot.as_ref().is_none_or(|previous| {
            previous.1 != active || previous.2 != snapshot.2 || previous.0.len() != entries.len()
        });
        strip.snapshot = Some(snapshot);
        for view in strip.document.subviews() {
            view.removeFromSuperview();
        }
        let width = (available / entries.len().max(1) as f64).max(120.0);
        strip.document.setFrameSize(NSSize::new(
            (width * entries.len() as f64).max(available),
            32.0,
        ));
        for (index, tab) in entries.iter().enumerate() {
            let selected = tab.label == active;
            let title = if tab.title.is_empty() {
                "New Tab"
            } else {
                &tab.title
            };
            let frame = NSRect::new(
                NSPoint::new(index as f64 * width, 0.0),
                NSSize::new(width, 32.0),
            );
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
            };
            let label = button(
                action.clone(),
                title,
                NSRect::new(NSPoint::new(32.0, 0.0), NSSize::new(width - 64.0, 32.0)),
                selected,
                mtm,
            );
            label.setToolTip(Some(&NSString::from_str(title)));
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
            cell.addSubview(&close);
            strip.document.addSubview(&cell);
            if selected && changed_selection {
                strip.document.scrollRectToVisible(frame);
            }
        }
    });
}

pub fn remove(window_label: &str) {
    STRIP.with(|slot| {
        slot.borrow_mut().remove(window_label);
    });
}
