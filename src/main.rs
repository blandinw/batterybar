#![allow(non_upper_case_globals)]

extern crate cocoa;
extern crate core_foundation;
extern crate core_foundation_sys;
extern crate env_logger;
#[macro_use]
extern crate log;
#[macro_use]
extern crate objc;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicyAccessory, NSButton, NSStatusBar,
    NSVariableStatusItemLength,
};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSFastEnumeration, NSString};
use core_foundation_sys::array::CFArrayRef;
use core_foundation_sys::base::CFTypeRef;
use core_foundation_sys::date::CFTimeInterval;
use core_foundation_sys::dictionary::CFDictionaryRef;
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel, YES};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::process::{Command, Output};
use std::result::Result;
use std::sync::Mutex;
use std::thread;
use std::time;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    /*
     * IOPowerSources.h
     */
    // https://developer.apple.com/documentation/iokit/iopskeys.h/defines?language=objc
    pub fn IOPSCopyPowerSourcesInfo() -> CFTypeRef;
    pub fn IOPSCopyPowerSourcesList(blob: CFTypeRef) -> CFArrayRef;
    pub fn IOPSGetPowerSourceDescription(blob: CFTypeRef, ps: CFTypeRef) -> CFDictionaryRef;
    pub fn IOPSGetTimeRemainingEstimate() -> CFTimeInterval;
}

pub const kIOPSACPowerValue: &str = "AC Power";
pub const kIOPSBatteryPowerValue: &str = "Battery Power";
pub const kIOPSIsChargingKey: &str = "Is Charging";
pub const kIOPSMaxCapacityKey: &str = "Max Capacity";
pub const kIOPSCurrentCapacityKey: &str = "Current Capacity";
pub const kIOPSPowerSourceStateKey: &str = "Power Source State";
pub const kIOPSTimeToEmptyKey: &str = "Time to Empty";
pub const kIOPSTimeToFullChargeKey: &str = "Time to Full Charge";
pub const kIOPSTimeRemainingUnknown: CFTimeInterval = -1.0;
pub const kIOPSTimeRemainingUnlimited: CFTimeInterval = -2.0;

static mut STATUS_ITEM: Option<id> = None;
const NOTIF_THRESHOLD: f64 = 5.;

unsafe fn status_item() -> id {
    STATUS_ITEM.unwrap()
}

pub struct FromId {
    id: id,
}

unsafe fn id_to_string(id: id) -> String {
    let chars: *const c_char = msg_send![id, UTF8String];
    let cstr = CStr::from_ptr(chars);
    cstr.to_owned().into_string().unwrap()
}

impl From<FromId> for String {
    fn from(x: FromId) -> Self {
        unsafe { id_to_string(x.id) }
    }
}

impl From<FromId> for i32 {
    fn from(x: FromId) -> Self {
        unsafe { msg_send![x.id, intValue] }
    }
}

impl From<FromId> for bool {
    fn from(x: FromId) -> Self {
        unsafe { YES == msg_send![x.id, boolValue] }
    }
}

fn nsdict_get<T: From<FromId>>(dict: id, key: &str) -> T {
    let k = unsafe { NSString::alloc(nil).init_str(key) };
    let v: FromId = unsafe { msg_send![dict, objectForKey: k] };
    From::from(v)
}

fn human_time(minutes: i64) -> String {
    return format!("{}:{:02}", minutes / 60, minutes % 60);
}

fn compute_title_and_percent() -> (String, f64) {
    let blob = unsafe { IOPSCopyPowerSourcesInfo() };
    let nsary: id = unsafe { IOPSCopyPowerSourcesList(blob) } as id;
    // TODO(willy) smarter power source selection using kIOPSInternalBatteryType
    let nsdict: id = unsafe { nsary.iter() }.next().unwrap();

    let state = nsdict_get::<String>(nsdict, kIOPSPowerSourceStateKey);
    let current_cap = nsdict_get::<i32>(nsdict, kIOPSCurrentCapacityKey);
    let max_cap = nsdict_get::<i32>(nsdict, kIOPSMaxCapacityKey);
    let current_pct = 100. * current_cap as f64 / max_cap as f64;

    #[allow(non_upper_case_globals)]
    let title = match state.as_ref() {
        kIOPSBatteryPowerValue => {
            let mins = nsdict_get::<i32>(nsdict, kIOPSTimeToEmptyKey);
            format!(
                "\u{2193} {}({}%)",
                if mins == 0 || mins == kIOPSTimeRemainingUnknown as i32 {
                    String::from("")
                } else {
                    format!("{} ", human_time(mins as i64))
                },
                current_pct
            )
        }
        kIOPSACPowerValue => {
            let mins = nsdict_get::<i32>(nsdict, kIOPSTimeToFullChargeKey);
            format!(
                "\u{2191} {}({}%)",
                if mins == 0 || mins == kIOPSTimeRemainingUnknown as i32 {
                    String::from("")
                } else {
                    format!("{} ", human_time(mins as i64))
                },
                current_pct
            )
        }
        &_ => String::from("\u{1F914}"),
    };

    (title, current_pct)
}

fn send_notification(msg: &str, say: &str, voice: &str) -> Result<Output, std::io::Error> {
    let notif_cmd = format!("display notification \"{}\" with title \"batterybar\"", msg);
    let say_cmd = format!("say \"{}\" -v \"{}\" -r 160", say, voice);
    Command::new("osascript").args(&["-e", &notif_cmd]).output()?;
    Command::new("bash").args(&["-c", &say_cmd]).output()
}

fn main() {
    env_logger::init();

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        // init NSApplication (order matters, do this first)
        let app = NSApp();

        // init static objects
        let status_bar = NSStatusBar::systemStatusBar(nil);
        STATUS_ITEM = Some(status_bar.statusItemWithLength_(NSVariableStatusItemLength));

        // customize NSApplication
        app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);

        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("MyAppDelegate", superclass).unwrap();

        // NSApplicationDelegate
        extern "C" fn application_did_finish_launching(_: &Object, _: Sel, _: id) {
            info!("application did finish launching");

            let sent_notif_mutex = Mutex::new(false);

            thread::spawn(move || loop {
                let (title, pct) = compute_title_and_percent();

                unsafe {
                    status_item().setTitle_(NSString::alloc(nil).init_str(&title).autorelease());
                }

                let mut sent_notif = sent_notif_mutex.lock().unwrap();
                if pct <= NOTIF_THRESHOLD && !*sent_notif {
                    let notif_msg = format!("Battery at {}%", pct);
                    let say_msg = format!("배터리가 {}% 입니다", pct);
                    let say_voice = "Yuna";
                    send_notification(&notif_msg, &say_msg, &say_voice)
                        .expect("could not display notification");
                    *sent_notif = true;
                } else if pct > NOTIF_THRESHOLD && *sent_notif {
                    *sent_notif = false;
                }

                thread::sleep(time::Duration::from_millis(5000));
            });
        }
        decl.add_method(
            sel!(applicationDidFinishLaunching:),
            application_did_finish_launching as extern "C" fn(&Object, Sel, id),
        );

        let delegate_class = decl.register();
        let delegate_object: id = msg_send![delegate_class, new];

        let _: () = msg_send![app, setDelegate: delegate_object];

        app.run();
    }
}
