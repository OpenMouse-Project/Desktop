//! Being told when a HID device is plugged in or removed, so the app can look
//! again without being asked.
//!
//! Windows reports it (`CM_Register_Notification`), which costs nothing while
//! idle. Elsewhere [`watch`] returns `None` and the app checks on a timer.

use futures::channel::mpsc::UnboundedReceiver;

/// One message per change. A single device sends several, one per interface.
/// Registered for the life of the process, so call it once.
#[cfg(windows)]
pub fn watch() -> Option<UnboundedReceiver<()>> {
    windows::register()
}

#[cfg(not(windows))]
pub fn watch() -> Option<UnboundedReceiver<()>> {
    None
}

#[cfg(windows)]
mod windows {
    use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
    use std::ffi::c_void;

    #[repr(C)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    /// `GUID_DEVINTERFACE_HID`.
    const HID: Guid = Guid {
        data1: 0x4D1E_55B2,
        data2: 0xF16F,
        data3: 0x11CF,
        data4: [0x88, 0xCB, 0x00, 0x11, 0x11, 0x00, 0x00, 0x30],
    };

    /// `CM_NOTIFY_FILTER`. Its union is as large as its biggest member, a
    /// 200-character instance id, of which a device interface filter uses only
    /// the class GUID at the start.
    #[repr(C)]
    struct Filter {
        size: u32,
        flags: u32,
        /// 0: device interfaces of one class.
        kind: u32,
        reserved: u32,
        class: Guid,
        rest: [u8; 400 - std::mem::size_of::<Guid>()],
    }

    const ARRIVAL: u32 = 0;
    const REMOVAL: u32 = 1;

    type Callback = unsafe extern "system" fn(*mut c_void, *const c_void, u32, *const c_void, u32) -> u32;

    #[link(name = "cfgmgr32")]
    extern "system" {
        fn CM_Register_Notification(
            filter: *const Filter,
            context: *const c_void,
            callback: Callback,
            handle: *mut *mut c_void,
        ) -> u32;
    }

    unsafe extern "system" fn changed(
        _handle: *mut c_void,
        context: *const c_void,
        action: u32,
        _data: *const c_void,
        _size: u32,
    ) -> u32 {
        if action == ARRIVAL || action == REMOVAL {
            // SAFETY: `context` is the sender leaked in `register`.
            let sender = unsafe { &*context.cast::<UnboundedSender<()>>() };
            let _ = sender.unbounded_send(());
        }
        0 // ERROR_SUCCESS
    }

    pub fn register() -> Option<UnboundedReceiver<()>> {
        let (sender, receiver) = mpsc::unbounded();
        // Never unregistered, so the sender has to outlive every callback.
        let context: *const UnboundedSender<()> = Box::leak(Box::new(sender));
        let filter = Filter {
            size: std::mem::size_of::<Filter>() as u32,
            flags: 0,
            kind: 0,
            reserved: 0,
            class: HID,
            rest: [0; 400 - std::mem::size_of::<Guid>()],
        };
        let mut handle = std::ptr::null_mut();
        // SAFETY: the filter matches the documented layout and is only read
        // during the call; the context is valid for the life of the process.
        let result = unsafe { CM_Register_Notification(&filter, context.cast(), changed, &mut handle) };
        if result != 0 {
            crate::logging::log(&format!("[device] could not watch for plug-ins: error {result}"));
            return None;
        }
        Some(receiver)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn windows_accepts_the_registration() {
            assert_eq!(std::mem::size_of::<super::Filter>(), 416);
            assert!(super::register().is_some());
        }
    }
}
