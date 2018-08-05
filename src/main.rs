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

#[allow(non_upper_case_globals)]
pub const kIOPSACPowerValue: &str = "AC Power";
#[allow(non_upper_case_globals)]
pub const kIOPSBatteryPowerValue: &str = "Battery Power";
#[allow(non_upper_case_globals)]
pub const kIOPSIsChargingKey: &str = "Is Charging";
#[allow(non_upper_case_globals)]
pub const kIOPSMaxCapacityKey: &str = "Max Capacity";
#[allow(non_upper_case_globals)]
pub const kIOPSCurrentCapacityKey: &str = "Current Capacity";
#[allow(non_upper_case_globals)]
pub const kIOPSPowerSourceStateKey: &str = "Power Source State";
#[allow(non_upper_case_globals)]
pub const kIOPSTimeToEmptyKey: &str = "Time to Empty";
#[allow(non_upper_case_globals)]
pub const kIOPSTimeToFullChargeKey: &str = "Time to Full Charge";
#[allow(non_upper_case_globals)]
pub const kIOPSTimeRemainingUnknown: CFTimeInterval = -1.0;
#[allow(non_upper_case_globals)]
pub const kIOPSTimeRemainingUnlimited: CFTimeInterval = -2.0;

static mut STATUS_ITEM: Option<id> = None;

pub struct FromId {
    id: id
}

impl From<FromId> for String {
    fn from(x: FromId) -> Self {
        let chars: *const c_char = unsafe { msg_send![x.id, UTF8String] };
        let cstr = unsafe { CStr::from_ptr(chars) };
        cstr.to_owned().into_string().unwrap()
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
    let v: FromId = unsafe { msg_send![dict, objectForKey:k] };
    From::from(v)
}

fn human_time(minutes: i64) -> String {
    let mut x = minutes;

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

fn generate_title() -> String {
    let blob = unsafe { IOPSCopyPowerSourcesInfo() };
    let nsary: id = unsafe { IOPSCopyPowerSourcesList(blob) } as id;
    // TODO(willy) smarter power source selection using kIOPSInternalBatteryType
    let nsdict: id = unsafe { nsary.iter() }.next().unwrap();

    let state = nsdict_get::<String>(nsdict, kIOPSPowerSourceStateKey);
    let current_cap = nsdict_get::<i32>(nsdict, kIOPSCurrentCapacityKey);
    let max_cap = nsdict_get::<i32>(nsdict, kIOPSMaxCapacityKey);
    let current_pct = 100. * current_cap as f64 / max_cap as f64;

    #[allow(non_upper_case_globals)]
        match state.as_ref() {
        kIOPSBatteryPowerValue => {
            let mins = nsdict_get::<i32>(nsdict, kIOPSTimeToEmptyKey);
            if mins == kIOPSTimeRemainingUnknown as i32 {
                format!("{}%", current_pct)
            } else {
                format!(
                    "{} ({}%)",
                    human_time(mins as i64),
                    current_pct
                )
            }
        }
        kIOPSACPowerValue => {
            let mins = nsdict_get::<i32>(nsdict, kIOPSTimeToFullChargeKey);
            if mins == kIOPSTimeRemainingUnknown as i32 {
                format!("{}%", current_pct)
            } else {
                format!(
                    "{} ({}%%)",
                    human_time(mins as i64),
                    current_pct
                )
            }
        }
        &_ => {
            String::from("\u{1F914}")
        }
    }
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

            thread::spawn(|| loop {
                let title = generate_title();
                unsafe {
                    STATUS_ITEM.unwrap().setTitle_(
                        NSString::alloc(nil).init_str(&title).autorelease()
                    );
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

        msg_send![app, setDelegate: delegate_object];

        app.run();
    }
}
