extern crate cocoa;
extern crate core_foundation_sys;
extern crate env_logger;
#[macro_use]
extern crate log;
#[macro_use]
extern crate objc;

#[allow(unused_imports)]
use cocoa::appkit::NSApplicationActivationPolicyRegular;
use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicyAccessory, NSButton, NSStatusBar,
    NSVariableStatusItemLength,
};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSArray, NSAutoreleasePool, NSString};
use core_foundation_sys::date::CFTimeInterval;
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use std::thread;
use std::time;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    /*
     * IOPowerSources.h
     */
    pub fn IOPSGetTimeRemainingEstimate() -> CFTimeInterval;
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {
    #[allow(non_upper_case_globals)]
    static NSTouchBarItemIdentifierCharacterPicker: id;
}

#[allow(non_upper_case_globals)]
pub const kIOPSTimeRemainingUnknown: CFTimeInterval = -1.0;
#[allow(non_upper_case_globals)]
pub const kIOPSTimeRemainingUnlimited: CFTimeInterval = -2.0;

fn human_time(seconds: i64) -> String {
    let mut x = seconds;

    x /= 60;
    let mins = x % 60;
    if x < 60 {
        return format!("{}m", mins);
    }

    x /= 60;
    let hours = x % 24;
    if x < 24 {
        return format!("{}h{}m", hours, mins);
    }

    x /= 24;
    format!("{}d{}h{}m", x, hours, mins)
}

static mut STATUS_ITEM: Option<id> = None;
static mut TOUCHBAR_ITEM_ID: id = nil;

fn main() {
    env_logger::init();

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        // init NSApplication (order matters, do this first)
        let app = NSApp();

        // init static objects
        TOUCHBAR_ITEM_ID =
            NSString::alloc(nil).init_str("com.github.blandinw.batterybar.touchbaritem1");
        let status_bar = NSStatusBar::systemStatusBar(nil);
        STATUS_ITEM = Some(status_bar.statusItemWithLength_(NSVariableStatusItemLength));

        // customize NSApplication
        // app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
        app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);

        // let img_path: id = NSString::alloc(nil).init_str("/Applications/Sketch.app/Contents/Resources/app.icns");
        // let mut img: id = msg_send![class!(NSImage), alloc];
        // img = msg_send![img, initByReferencingFile: img_path];
        // msg_send![app, setApplicationIconImage: img];

        let superclass = class!(NSResponder);
        let mut decl = ClassDecl::new("MyAppDelegate", superclass).unwrap();

        // NSApplicationDelegate
        extern "C" fn application_did_finish_launching(_: &Object, _: Sel, _: id) {
            info!("application did finish launching");

            thread::spawn(|| loop {
                let remaining_secs = unsafe { IOPSGetTimeRemainingEstimate() };
                let label = if remaining_secs == kIOPSTimeRemainingUnknown {
                    String::from("\u{1F914}")
                } else if remaining_secs == kIOPSTimeRemainingUnlimited {
                    String::from("\u{1F60E}")
                } else {
                    human_time(remaining_secs as i64)
                };
                debug!("{} -> {}", remaining_secs, label);

                unsafe {
                    let title = NSString::alloc(nil).init_str(&label).autorelease();
                    STATUS_ITEM.unwrap().setTitle_(title);
                }

                thread::sleep(time::Duration::from_millis(5000));
            });
        }
        decl.add_method(
            sel!(applicationDidFinishLaunching:),
            application_did_finish_launching as extern "C" fn(&Object, Sel, id),
        );

        // NSTouchBarProvider
        extern "C" fn make_touchbar(_this: &mut Object, _: Sel) -> id {
            info!("make touchbar");
            unsafe {
                let touchbar: id = msg_send![class!(NSTouchBar), new];
                let title: id = NSString::alloc(nil).init_str("Hello Lin~ \u{1F430}");
                let button: id =
                    msg_send![class!(NSButton), buttonWithTitle:title target:nil action:nil];
                let mut item: id = msg_send![class!(NSCustomTouchBarItem), alloc];
                item = msg_send![item, initWithIdentifier: TOUCHBAR_ITEM_ID];
                msg_send![item, setView: button];

                let items_array: id = NSArray::arrayWithObject(nil, item);
                let items_set: id = msg_send![class!(NSSet), setWithArray: items_array];

                let item_identifiers: id = NSArray::arrayWithObjects(
                    nil,
                    &[NSTouchBarItemIdentifierCharacterPicker, TOUCHBAR_ITEM_ID],
                );

                msg_send![touchbar, setTemplateItems: items_set];
                msg_send![touchbar, setDefaultItemIdentifiers: item_identifiers];

                return touchbar;
            };
        }
        decl.add_method(
            sel!(makeTouchBar),
            make_touchbar as extern "C" fn(&mut Object, Sel) -> id,
        );

        let delegate_class = decl.register();
        let delegate_object: id = msg_send![delegate_class, new];

        msg_send![app, setDelegate: delegate_object];

        app.run();
    }
}
