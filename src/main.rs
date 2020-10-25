#![allow(non_upper_case_globals)]

extern crate cocoa;
extern crate core_foundation;
extern crate core_foundation_sys;
#[macro_use]
extern crate objc;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicyAccessory, NSStatusBar,
    NSVariableStatusItemLength, NSWindow,
};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSString};
use core_foundation_sys::array::{CFArrayRef, CFArrayGetValueAtIndex};
use core_foundation_sys::base::{CFRelease, CFTypeRef};
use core_foundation_sys::date::CFTimeInterval;
use core_foundation_sys::dictionary::CFDictionaryRef;
use objc::declare::ClassDecl;
use objc::rc::autoreleasepool;
use objc::runtime;
use objc::runtime::{Class, Object, Sel};
use std::ffi::{c_void, CStr};
use std::marker::PhantomData;
use std::ops::Deref;
use std::os::raw::c_char;
use std::process::{Command, Output};
use std::result::Result;
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

const NOTIF_THRESHOLD: f64 = 5.;

struct SendableId(id);
impl SendableId {
    unsafe fn retain(ptr: id) -> Self {
        runtime::objc_retain(ptr);
        SendableId(ptr)
    }
}
impl Deref for SendableId {
    type Target = id;
    fn deref(&self) -> &id {
        &self.0
    }
}
impl Drop for SendableId {
    fn drop(&mut self) {
        unsafe {
            runtime::objc_release(self.0);
        }
    }
}
unsafe impl Send for SendableId {}

struct CFReleaser(CFTypeRef);

impl Drop for CFReleaser {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.0);
        }
    }
}

struct Id(id);

unsafe fn id_to_string(id: id) -> String {
    let chars: *const c_char = msg_send![id, UTF8String];
    let cstr = CStr::from_ptr(chars);
    cstr.to_owned().into_string().unwrap()
}

impl From<Id> for String {
    fn from(x: Id) -> Self {
        unsafe { id_to_string(x.0) }
    }
}

impl From<Id> for i32 {
    fn from(x: Id) -> Self {
        unsafe { msg_send![x.0, intValue] }
    }
}

trait Fromable {
    fn fromable(x: &Id, PhantomData<Self>) -> bool;
}

impl Fromable for String {
    fn fromable(x: &Id, _: PhantomData<Self>) -> bool {
        unsafe {
            let c: &Class = msg_send![x.0, class];
            msg_send![c, isSubclassOfClass: class!(NSString)]
        }
    }
}

impl Fromable for i32 {
    fn fromable(x: &Id, _: PhantomData<Self>) -> bool {
        unsafe {
            let c: &Class = msg_send![x.0, class];
            msg_send![c, isSubclassOfClass: class!(NSNumber)]
        }
    }
}

fn nsdict_get<T: From<Id> + Fromable>(dict: id, key: &str) -> Result<T, String> {
    let k = unsafe { NSString::alloc(nil).init_str(key) };
    let v: Id = unsafe { msg_send![dict, objectForKey: k] };
    let r: bool = Fromable::fromable(&v, PhantomData as PhantomData<T>);
    if r {
        Ok(From::from(v))
    } else {
        Err(String::from("cannot convert to desired type"))
    }
}

fn human_time(minutes: i64) -> String {
    return format!("{}:{:02}", minutes / 60, minutes % 60);
}

fn compute_title_and_percent() -> (String, f64) {
    let blob = unsafe { IOPSCopyPowerSourcesInfo() };
    let _1 = CFReleaser(blob);
    let sources = unsafe { IOPSCopyPowerSourcesList(blob)};
    let _2 = CFReleaser(sources as *const c_void);
    // TODO(willy) smarter power source selection using kIOPSInternalBatteryType
    let ps = unsafe { CFArrayGetValueAtIndex(sources, 0) } as id;

    let state = nsdict_get::<String>(ps, kIOPSPowerSourceStateKey).unwrap();
    let current_cap = nsdict_get::<i32>(ps, kIOPSCurrentCapacityKey).unwrap();
    let max_cap = nsdict_get::<i32>(ps, kIOPSMaxCapacityKey).unwrap();
    let current_pct = 100. * current_cap as f64 / max_cap as f64;

    let title = match state.as_ref() {
        kIOPSBatteryPowerValue => {
            let mins = nsdict_get::<i32>(ps, kIOPSTimeToEmptyKey).unwrap();
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
            let mins = nsdict_get::<i32>(ps, kIOPSTimeToFullChargeKey).unwrap();
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
    Command::new("osascript")
        .args(&["-e", &notif_cmd])
        .output()?;
    Command::new("bash").args(&["-c", &say_cmd]).output()
}

extern "C" fn application_did_finish_launching(_: &Object, _: Sel, _: id) {
    let status_item = unsafe {
        SendableId::retain(
            NSStatusBar::systemStatusBar(nil).statusItemWithLength_(NSVariableStatusItemLength),
        )
    };

    thread::spawn(move || {
        let mut sent_notif = false;

        loop {
            autoreleasepool(|| {
                let (title, pct) = compute_title_and_percent();

                unsafe {
                    let nstitle = NSString::alloc(nil).init_str(&title).autorelease();
                    status_item.setTitle_(nstitle);
                }

                if pct <= NOTIF_THRESHOLD && !sent_notif {
                    let notif_msg = format!("Battery at {}%", pct);
                    let say_msg = format!("배터리가 {}% 입니다", pct);
                    let say_voice = "Yuna";
                    send_notification(&notif_msg, &say_msg, &say_voice)
                        .expect("could not display notification");
                    sent_notif = true;
                } else if pct > NOTIF_THRESHOLD && sent_notif {
                    sent_notif = false;
                }
            });

            thread::sleep(time::Duration::from_millis(10000));
        }
    });
}

fn main() {
    unsafe {
        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);

        let delegate_class = {
            let mut decl = ClassDecl::new("BatterybarAppDelegate", class!(NSObject)).unwrap();
            decl.add_method(
                sel!(applicationDidFinishLaunching:),
                application_did_finish_launching as extern "C" fn(&Object, Sel, id),
            );
            decl.register()
        };
        let delegate_object: id = msg_send![delegate_class, new];

        app.setDelegate_(delegate_object);
        app.run();
    }
}
