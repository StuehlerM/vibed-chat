// Native-control accessibility annotations and coalesced state notifications.
// Win32/RichEdit supply the underlying editor Value/Text and button providers.
#![allow(dead_code)]

use super::ffi::*;
use std::ptr;

const CLASS: GUID = GUID {
    data1: 0xb5f8350b,
    data2: 0x0548,
    data3: 0x48b1,
    data4: [0xa6, 0xee, 0x88, 0xbd, 0x00, 0xb4, 0xa5, 0xe7],
};
const INTERFACE: GUID = GUID {
    data1: 0x6e26e776,
    data2: 0x04f0,
    data3: 0x495d,
    data4: [0x80, 0xe4, 0x33, 0x30, 0x35, 0x2e, 0x31, 0x69],
};
const NAME: GUID = GUID {
    data1: 0x608d3df8,
    data2: 0x8128,
    data3: 0x4aa7,
    data4: [0xa4, 0x28, 0xf5, 0x5e, 0x49, 0x26, 0x72, 0x91],
};
const DESCRIPTION: GUID = GUID {
    data1: 0x4d48dfe4,
    data2: 0xbd3f,
    data3: 0x491f,
    data4: [0xa6, 0x48, 0x49, 0x2d, 0x6f, 0x20, 0xc5, 0x88],
};
const OBJID_CLIENT: u32 = (-4i32) as u32;

struct Services(*mut ACC_PROP_SERVICES);
impl Services {
    fn new() -> Option<Self> {
        // SAFETY: OLE is initialized by the UI-thread entry point. The requested
        // interface is IAccPropServices, and its reference is released below.
        unsafe {
            let mut result = ptr::null_mut();
            let hr = CoCreateInstance(&CLASS, ptr::null_mut(), 1, &INTERFACE, &mut result);
            if hr < 0 || result.is_null() {
                None
            } else {
                Some(Self(result.cast()))
            }
        }
    }
}
impl Drop for Services {
    fn drop(&mut self) {
        // SAFETY: one owned COM reference, used/released only on its UI thread.
        unsafe {
            ((*(*self.0).vtable).release)(self.0);
        }
    }
}

pub fn name(hwnd: HWND, label: &str, description: &str) -> bool {
    let Some(service) = Services::new() else {
        return false;
    };
    let label = to_wide(label);
    let description = to_wide(description);
    // SAFETY: HWND is a live native child; strings live through the synchronous
    // annotation calls. No AppState references cross COM/native callbacks.
    unsafe {
        let table = &*(*service.0).vtable;
        let a = (table.set_hwnd_prop_str)(service.0, hwnd, OBJID_CLIENT, 0, NAME, label.as_ptr());
        let b = (table.set_hwnd_prop_str)(
            service.0,
            hwnd,
            OBJID_CLIENT,
            0,
            DESCRIPTION,
            description.as_ptr(),
        );
        a >= 0 && b >= 0
    }
}

pub fn clear(hwnd: HWND) {
    let Some(service) = Services::new() else {
        return;
    };
    let properties = [NAME, DESCRIPTION];
    // SAFETY: remove only our annotations before HWND destruction/reuse.
    unsafe {
        ((*(*service.0).vtable).clear_hwnd_props)(
            service.0,
            hwnd,
            OBJID_CLIENT,
            0,
            properties.as_ptr(),
            properties.len() as i32,
        );
    }
}

/// Only call for discrete state changes, never for each streamed token or
/// hidden reasoning. Native text controls deliver their own editing events.
pub fn state_changed(hwnd: HWND) {
    // SAFETY: event describes the live native status control, without borrowing
    // application state or submitting any command in an accessibility callback.
    unsafe {
        NotifyWinEvent(0x800c, hwnd, -4, 0);
    } // EVENT_OBJECT_NAMECHANGE
}
