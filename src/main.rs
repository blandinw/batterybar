extern crate cocoa;
extern crate core_foundation_sys;
extern crate env_logger;
#[macro_use]
extern crate log;
#[macro_use]
extern crate objc;

use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicyRegular, NSStatusBar, NSVariableStatusItemLength, NSButton};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSString};
use core_foundation_sys::date::CFTimeInterval;
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use std::thread;
use std::time;

#[link(name = "IOKit", kind = "framework")]
extern {
    /*
     * IOPowerSources.h
     */
    pub fn IOPSGetTimeRemainingEstimate() -> CFTimeInterval;
}

#[allow(non_upper_case_globals)]
pub const kIOPSTimeRemainingUnknown: CFTimeInterval = -1.0;
#[allow(non_upper_case_globals)]
pub const kIOPSTimeRemainingUnlimited: CFTimeInterval = -2.0;

fn set_delegate(x: id, delegate_object: id) {
    unsafe {
        msg_send![x, setDelegate:delegate_object];
    }
}

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

fn main() {
    env_logger::init();

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

        let status_bar = NSStatusBar::systemStatusBar(nil);
        STATUS_ITEM = Some(status_bar.statusItemWithLength_(NSVariableStatusItemLength));

        // Create NSApplicationDelegate
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("MyAppDelegate", superclass).unwrap();

        extern fn application_did_finish_launching(_: &Object, _: Sel, _: id) {
            info!("application did finish launching");
            loop {
                let remaining_secs = unsafe { IOPSGetTimeRemainingEstimate() };
                let label = if remaining_secs == kIOPSTimeRemainingUnknown {
                    String::from("unk")
                } else if remaining_secs == kIOPSTimeRemainingUnlimited {
                    String::from("inf")
                } else {
                    human_time(remaining_secs as i64)
                };
                debug!("{} -> {}", remaining_secs, label);

                unsafe {
                    let title = NSString::alloc(nil).init_str(&label).autorelease();
                    STATUS_ITEM.unwrap().setTitle_(title);
                }

                thread::sleep(time::Duration::from_millis(30000));
            }
        }

        decl.add_method(sel!(applicationDidFinishLaunching:),
                        application_did_finish_launching as extern fn(&Object, Sel, id));

        let delegate_class = decl.register();
        let delegate_object = msg_send![delegate_class, new];

        set_delegate(app, delegate_object);

        app.run();
    }
}