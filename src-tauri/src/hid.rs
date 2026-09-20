//! Native HID access for Full Desktop Mode, exposed to the frontend as Tauri
//! commands + an `"hid-input-report"` event.
//!
//! This exists so the frontend can drive `@openmouse/protocol`'s driver
//! classes — written against the browser WebHID API — from inside Tauri's
//! webview, which does not implement WebHID. The frontend's
//! `TauriHidDevice` (see `src/native-hid/tauri-hid-device.ts`) wraps these
//! commands into the same `HIDDevice`-shaped interface the driver classes
//! expect, mirroring OpenMouse-Bridge's `native-hid/src/hid-device-adapter.mjs`
//! (which does the same thing for a Node host instead of a webview).
//!
//! One WebHID `HIDDevice` == one USB/BT *interface*, which can carry several
//! top-level HID collections (e.g. a short and a long report variant on the
//! same vendor interface). hidapi enumerates one entry *per collection*, not
//! per interface, so every collection sharing a (vendor id, product id) pair
//! is grouped and opened together as one logical device.
//!
//! This used to group by (vendor id, product id, interface number) instead —
//! mirroring the Node adapter's `candidateDevices()` — but `interface_number`
//! turned out to not be stable across separate `device_list()` calls on
//! macOS for several real devices (a Logitech receiver, a Wooting keyboard),
//! producing a different number per enumeration for what was the same
//! physical collection. That surfaced as duplicate rows in the device list
//! (the same physical device appearing under several different keys) and, on
//! connect, an "exclusive access, device already open" hidapi error (list
//! time and open time disagreeing on which path a given key meant). Vendor +
//! product id alone is coarser — two genuinely different physical devices
//! that happen to share both ids would incorrectly merge into one entry —
//! but that's a rare edge case, and it's stable, which interface_number is
//! not on this platform.
//!
//! ## The actual cause of the input-freeze bug (found via the hidapi crate's
//! own doc comment on `HidApi::set_open_exclusive`, after several wrong
//! guesses — see git history on this file for the discarded theories):
//! **on macOS, hidapi opens every device in *exclusive* mode by default.**
//! That seizes the device from the OS's own HID client, which is exactly
//! why any real mouse/keyboard went unresponsive the moment this code
//! opened it, regardless of which collection, how long it stayed open, or
//! whether the `HidApi` instance was shared or freshly created each call —
//! none of those things actually mattered; every one of them still opened
//! in exclusive mode. `with_hid_api()` below calls
//! `api.set_open_exclusive(false)` once, immediately after creating the
//! shared instance, so every subsequent `open_path()` opens non-exclusively
//! and coexists with the OS's own handling of the device.
//!
//! **Known gap, confirmed on real hardware, not yet solved:** at least one
//! device (Endgame Gear's OP1-8K) answers `GetFeatureReport` with an I/O
//! timeout when opened non-exclusively — its control-transfer protocol
//! appears to require exclusive HID access to work at all. Exclusive access
//! was tried again with the shortest possible open/one-read/close window
//! and still froze the device's own input immediately, so duration is not
//! a mitigating factor. Non-exclusive stays the default because freezing a
//! user's real mouse/keyboard is a worse failure than a device's config
//! protocol not answering — a device that needs exclusive access to
//! function is presently unreachable through this bridge, not a bug to
//! "fix" by bringing exclusive mode back.
//!
//! A single `HidApi` instance is kept alive for the app's whole lifetime
//! (`HidApiHandle` below) and refreshed rather than recreated on every
//! call — simpler and cheaper than a fresh instance per call, and (now that
//! exclusive mode is off) has no bearing on the freeze either way.
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use hidapi::HidApi;
use parking_lot::Mutex;
use serde::Serialize;

use crate::hid_descriptor::{parse_report_descriptor, CollectionInfo};
use tauri::Emitter;

// ---------------------------------------------------------------------------
// Windows sub-collection enumeration
// ---------------------------------------------------------------------------
//
// hidapi on Windows returns one entry per *top-level* HID collection, but
// composite devices (Razer mice, for instance) can place their vendor
// protocol on a *sub*-collection that shares the same USB interface.
// `open_path` associates the handle with the *first* top-level collection,
// so feature reports declared on a later sub-collection are unreachable
// through that handle — `IOCTL_HID_GET_FEATURE` returns ERROR_INVALID_FUNCTION.
//
// The browser's WebHID sidesteps this: the browser parses every collection's
// report descriptor and routes reports to the correct one.  We replicate
// that by using `SetupDi*` APIs to enumerate every device-interface path
// the HID minidriver created (including sub-collection paths with `&colNN`
// in the device-id), then opening them all so `try_each` can probe each one.
// ---------------------------------------------------------------------------
#[cfg(target_os = "windows")]
mod windows_hid_enumerate {
    use std::ffi::c_void;
    use std::mem;
    use std::ptr;

    type BOOL = i32;
    type DWORD = u32;
    type HANDLE = *mut c_void;
    type HDEVINFO = *mut c_void;
    type PCWSTR = *const u16;

    const FALSE: BOOL = 0;
    const DIGCF_PRESENT: DWORD = 0x00000002;
    const DIGCF_DEVICEINTERFACE: DWORD = 0x00000010;
    const INVALID_HANDLE_VALUE: HANDLE = -1isize as *mut c_void;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct GUID {
        Data1: u32,
        Data2: u16,
        Data3: u16,
        Data4: [u8; 8],
    }

    // HID interface class GUID
    static HID_GUID: GUID = GUID {
        Data1: 0x4D1E55B2,
        Data2: 0xF16F,
        Data3: 0x11CF,
        Data4: [0x88, 0xCB, 0x00, 0x11, 0x11, 0x00, 0x00, 0x30],
    };

    #[repr(C)]
    #[allow(non_snake_case)]
    struct SP_DEVICE_INTERFACE_DATA {
        cbSize: DWORD,
        InterfaceClassGuid: GUID,
        Flags: DWORD,
        Reserved: usize,
    }

    #[repr(C)]
    #[allow(dead_code, non_snake_case)]
    struct SP_DEVINFO_DATA {
        cbSize: DWORD,
        ClassGuid: GUID,
        DevInst: DWORD,
        Reserved: usize,
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct SP_DEVICE_INTERFACE_DETAIL_DATA_W {
        cbSize: DWORD,
    }

    extern "system" {
        fn SetupDiGetClassDevsW(
            ClassGuid: *const GUID,
            Enumerator: PCWSTR,
            hwndParent: *mut c_void,
            Flags: DWORD,
        ) -> HDEVINFO;
        fn SetupDiEnumDeviceInterfaces(
            DeviceInfoSet: HDEVINFO,
            DeviceInfoData: *const SP_DEVINFO_DATA,
            InterfaceClassGuid: *const GUID,
            MemberIndex: DWORD,
            DeviceInterfaceData: *mut SP_DEVICE_INTERFACE_DATA,
        ) -> BOOL;
        fn SetupDiGetDeviceInterfaceDetailW(
            DeviceInfoSet: HDEVINFO,
            DeviceInterfaceData: *const SP_DEVICE_INTERFACE_DATA,
            DeviceInterfaceDetailData: *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W,
            DeviceInterfaceDetailDataSize: DWORD,
            RequiredSize: *mut DWORD,
            DeviceInfoData: *mut SP_DEVINFO_DATA,
        ) -> BOOL;
        fn SetupDiDestroyDeviceInfoList(DeviceInfoSet: HDEVINFO) -> BOOL;
    }

    /// Enumerate every HID device-interface path for the given `(vid, pid)`,
    /// including sub-collection paths (`&colNN` in the device-id) that
    /// hidapi's own enumeration may skip.
    pub fn enumerate_paths(vid: u16, pid: u16) -> Vec<String> {
        let mut paths = Vec::new();
        unsafe {
            let dev_info = SetupDiGetClassDevsW(
                &HID_GUID,
                ptr::null(),
                ptr::null_mut(),
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            );
            if dev_info == INVALID_HANDLE_VALUE {
                return paths;
            }
            let mut idx: DWORD = 0;
            loop {
                let mut iface: SP_DEVICE_INTERFACE_DATA = mem::zeroed();
                iface.cbSize = mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as DWORD;
                if SetupDiEnumDeviceInterfaces(
                    dev_info,
                    ptr::null(),
                    &HID_GUID,
                    idx,
                    &mut iface,
                ) == FALSE
                {
                    break;
                }
                idx += 1;
                let mut required: DWORD = 0;
                SetupDiGetDeviceInterfaceDetailW(
                    dev_info,
                    &iface,
                    ptr::null_mut(),
                    0,
                    &mut required,
                    ptr::null_mut(),
                );
                if required == 0 {
                    continue;
                }
                let mut buf: Vec<u8> = vec![0u8; required as usize];
                let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
                (*detail).cbSize = mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as DWORD;
                if SetupDiGetDeviceInterfaceDetailW(
                    dev_info,
                    &iface,
                    detail,
                    required,
                    ptr::null_mut(),
                    ptr::null_mut(),
                ) == FALSE
                {
                    continue;
                }
                let path_ptr = buf[mem::size_of::<DWORD>()..].as_ptr() as *const u16;
                let max_chars = (buf.len() - mem::size_of::<DWORD>()) / 2;
                let len = (0..max_chars)
                    .position(|i| *path_ptr.add(i) == 0)
                    .unwrap_or(max_chars);
                let path = String::from_utf16_lossy(std::slice::from_raw_parts(path_ptr, len));
                let lower = path.to_lowercase();
                if lower.contains(&format!("vid_{vid:04x}"))
                    && lower.contains(&format!("pid_{pid:04x}"))
                {
                    paths.push(path);
                }
            }
            SetupDiDestroyDeviceInfoList(dev_info);
        }
        paths
    }
}

/// Razer's USB vendor id. Used only for the write fan-out in `send_each`, which
/// mirrors the one brand OpenMouse-Bridge special-cases: it routes by declared
/// report ids wherever a report descriptor parses, and for Razer (whose
/// descriptors it does not inspect) writes to every interface instead.
const RAZER_VENDOR_ID: u16 = 0x1532;

/// How long each blocking read waits before checking the stop flag again.
///
/// This is also the floor on how long a *write* waits, which is why it is
/// short. `hidapi::HidDevice` is `Send` but not `Sync`, so the reader thread
/// and the request path (`try_each`) have to share one mutex, and the reader
/// holds it for the whole blocking read — a request can only be written once
/// that read returns.
///
/// MEASURED on real hardware (a Lightspeed receiver, with `try_each`'s own
/// lock wait instrumented): at the 200 ms this used to be, the 27-request
/// stretch of a `readStatus()` walk that was captured showed 24 separate
/// acquisitions at `wait_ms≈203` before their request could even be written
/// — only the 3 that happened to land in the idle gap below went out
/// immediately. A full 37-request Logitech walk is therefore seconds of pure
/// mutex wait before any protocol time at all. At 10 ms the same walk's worst
/// case is 37 * 10 ms, and polling this often costs nothing while idle:
/// hidapi's macOS read is a condition-variable wait woken by the report
/// callback, not a spin, so a shorter window only means returning earlier
/// when there is genuinely nothing queued.
const READ_POLL_TIMEOUT_MS: i32 = 10;

/// Gap between read iterations, held *outside* the device lock so a writer
/// parked on `try_each`'s blocking `lock()` can take it. Without this the
/// reader re-acquires via `try_lock()` the instant the previous guard drops
/// and the parked writer has no fairness guarantee against it — macOS's
/// pthread mutex (which parking_lot wraps here) does not queue a parked
/// waiter ahead of a thread that keeps re-acquiring, so the reader could win
/// every re-lock race indefinitely. That is a livelock, not a slow poll, and
/// it was seen live: a write for deviceIndex=0xff sat on "waiting for
/// lock…" forever while the reader looped. The gap is what actually hands the
/// lock over, so it stays on every iteration — not only after a failed
/// `try_lock()` — but it now runs *after* an incoming report has been
/// emitted, so it no longer delays report delivery by 5 ms.
const READ_IDLE_GAP_MS: u64 = 5;

#[derive(Clone, Serialize)]
pub struct HidInterface {
    pub key: String,
    #[serde(rename = "vendorId")]
    pub vendor_id: u16,
    #[serde(rename = "productId")]
    pub product_id: u16,
    #[serde(rename = "productString")]
    pub product_string: String,
    #[serde(rename = "manufacturerString")]
    pub manufacturer_string: String,
    /// This interface's HID collections and the report ids they declare, in
    /// WebHID's own shape. `@openmouse/protocol`'s drivers match devices with
    /// `isSupported(device)`, every one of which reads `device.collections` —
    /// a browser populates that for free, hidapi does not, which is why
    /// `src/native-hid/brands.ts` had to hand-copy the library's registry.
    /// Supplying it is what lets the app read the protocols from the library.
    pub collections: Vec<CollectionInfo>,
}

#[derive(Clone, Serialize)]
struct HidInputReportPayload {
    key: String,
    #[serde(rename = "reportId")]
    report_id: u8,
    data: Vec<u8>,
}

struct OpenSplit {
    device: Mutex<hidapi::HidDevice>,
    stop: Arc<AtomicBool>,
}

struct OpenGroup {
    splits: Vec<Arc<OpenSplit>>,
    readers: Vec<JoinHandle<()>>,
    /// Which split answered a given report id last time. WebHID never has to
    /// guess this — the browser already knows which collection declares a
    /// report id from `device.collections` and routes straight to it.
    /// hidapi's flat device list gives us no such map, so `try_each` used to
    /// re-probe from split 0 on every single call — for a device where the
    /// answering split isn't first, that's a guaranteed-fail write before
    /// the one that actually works, paid again on every one of the dozens of
    /// requests a full `readStatus()` walk makes. Once a split answers for a
    /// report id, remember it and try that one first next time.
    routes: Mutex<HashMap<u8, usize>>,
    /// Splits that accepted a request and answered with an empty packet —
    /// an interface that is up but not carrying the device's traffic (the
    /// usual cause: a composite wireless receiver, where the first split by
    /// rank is not the one the mouse answers on). They are skipped by
    /// `try_each` and never cached in `routes`; see the note in
    /// `hid_get_feature_report` for the failure this fixes. Cleared when a
    /// device is replugged, handled in `try_each`.
    silent: Mutex<HashSet<usize>>,
    /// The payload last written per report id, so a read that comes back as
    /// our own bytes can be told from an answer — see `is_unprocessed_echo`.
    last_sent: Mutex<HashMap<u8, Vec<u8>>>,
}

/// One `HidApi` for the app's lifetime — see the module docs above for why
/// this matters, not just for efficiency.
pub struct HidApiHandle(Mutex<Option<HidApi>>);

impl Default for HidApiHandle {
    fn default() -> Self {
        HidApiHandle(Mutex::new(None))
    }
}

#[derive(Default)]
pub struct HidRegistry(Mutex<HashMap<(u16, u16), OpenGroup>>);

/// Extra device-interface paths discovered on Windows via `SetupDi*` that
/// hidapi's own enumeration missed (sub-collection paths with `&colNN` in
/// the device-id).  Populated by `hid_list_interfaces`, consumed by
/// `hid_open`.
#[cfg(target_os = "windows")]
static EXTRA_WINDOWS_PATHS: LazyLock<Mutex<HashMap<(u16, u16), Vec<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn interface_key(vendor_id: u16, product_id: u16) -> String {
    format!("{vendor_id:04x}:{product_id:04x}")
}

/// The one message the frontend reads as "someone else is using this device,
/// try again" — byte-identical to BUSY_MESSAGE in
/// src/native-hid/hid-open-lock.ts, which is what makes a refusal here benign
/// (the auto-refresh and transport ticks retry on their own) instead of an
/// error the user is shown.
const HID_BUSY: &str = "This device is already busy with another request. Try again in a moment.";

/// Descriptor ceiling, matching hidapi's own bound.
const MAX_DESCRIPTOR_BYTES: usize = hidapi::MAX_REPORT_DESCRIPTOR_SIZE;

/// Collections parsed from each interface's report descriptor, cached by path.
/// Enumeration runs on every transport scan (seconds apart) and reading a
/// descriptor means opening the interface, so it is done once per path.
static DESCRIPTORS: LazyLock<Mutex<HashMap<String, Vec<CollectionInfo>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The collections of one interface group, merged across its paths — the app
/// presents a device's interfaces as one synthetic device (see
/// `tauri-hid-device.ts`), so this is that device's `collections`.
///
/// Best-effort: an interface whose descriptor cannot be read, or does not
/// parse, contributes nothing, which callers treat exactly as they treat a
/// device the protocol drivers do not recognise.
fn interface_collections(api: &HidApi, infos: &[&hidapi::DeviceInfo]) -> Vec<CollectionInfo> {
    let mut merged: Vec<CollectionInfo> = Vec::new();
    for info in infos {
        let path = info.path().to_string_lossy().into_owned();
        if let Some(cached) = DESCRIPTORS.lock().get(&path) {
            merged.extend(cached.iter().cloned());
            continue;
        }
        // A brief open, closed by the drop below: it is the only way to ask an
        // interface what it declares. This is safe to do during a scan now that
        // `with_hid_api` refuses a re-entrant call instead of blocking on it —
        // the open pumps the main thread's run loop (hidapi's macOS enumerator
        // does), which is what used to re-enter and deadlock the UI.
        //
        // Every failure is logged rather than swallowed: if a platform cannot
        // read descriptors at all, the drivers' collection-based `isSupported`
        // cannot fire either, and that has to be visible rather than showing up
        // as a device nothing claims.
        let parsed = {
            let mut buffer = vec![0u8; MAX_DESCRIPTOR_BYTES];
            match api.open_path(&info.path()) {
                Err(error) => {
                    applog!("[hid] descriptor for {path}: open failed: {error}");
                    None
                }
                Ok(device) => match device.get_report_descriptor(&mut buffer) {
                    Err(error) => {
                        applog!("[hid] descriptor for {path}: unreadable: {error}");
                        None
                    }
                    Ok(read) => match parse_report_descriptor(&buffer[..read]) {
                        Err(error) => {
                            applog!("[hid] descriptor for {path}: unparsable: {error}");
                            None
                        }
                        Ok((collections, _layout)) => Some(collections),
                    },
                },
            }
        };
        let collections = parsed.unwrap_or_default();
        if !collections.is_empty() {
            applog!(
                "[hid] descriptor for {path}: {} collection(s)",
                collections.len()
            );
        }
        DESCRIPTORS.lock().insert(path, collections.clone());
        merged.extend(collections);
    }
    merged
}

/// Runs `f` against the shared `HidApi`, initializing it on first use and
/// refreshing its device list (cheap — no manager teardown/recreation) on
/// every later call. Non-exclusive open is set once, right after creation —
/// see the module docs above for why this is the actual freeze fix.
///
/// `try_lock`, never `lock`: hidapi's macOS enumerator pumps the *calling
/// thread's* run loop while it waits for IOKit (`process_pending_events` →
/// `CFRunLoopRunInMode`, hid.c:492), and both callers here run on the main
/// thread. That pumped run loop services the webview's next `invoke`, which
/// lands back in this function on the same thread while the outer call still
/// holds the guard — so a blocking `lock()` can never succeed on that path.
/// It deadlocked exactly that way: `sample` of the frozen app showed the main
/// thread parked in `__psynch_cvwait` under `hid_list_interfaces` →
/// `with_hid_api` → `lock_slow`, nested inside `hid_open` →
/// `refresh_devices` → `hid_enumerate` → `CFRunLoopRunInMode`, with the
/// window painted but not responding and the mouse still attached. Reporting
/// BUSY instead unwinds the nested run loop so the outer call can finish, and
/// "busy" is the truth: this process is already mid-enumeration on the device.
fn with_hid_api<T>(
    handle: &HidApiHandle,
    f: impl FnOnce(&mut HidApi) -> Result<T, String>,
) -> Result<T, String> {
    let Some(mut guard) = handle.0.try_lock() else {
        applog!("[hid] with_hid_api: re-entered while an enumeration was in flight");
        return Err(HID_BUSY.to_string());
    };
    match guard.as_mut() {
        Some(api) => {
            api.refresh_devices().map_err(|error| error.to_string())?;
        }
        None => {
            let api = HidApi::new().map_err(|error| error.to_string())?;
            // CONFIRMED on real hardware: exclusive open freezes the
            // device's own input (mouse cursor/keyboard keys) the moment
            // it's opened — reproduced even with the shortest possible
            // open/one-read/close window, so duration was never the
            // mitigating factor. Non-exclusive avoids that entirely. The
            // real cost: at least one device's (Endgame Gear OP1-8K)
            // GetFeatureReport-based protocol needs exclusive access to
            // answer at all and fails outright non-exclusively — a genuine
            // conflict between "don't freeze the user's hardware" and
            // "every protocol variant works." Freezing real input is the
            // worse failure mode, so non-exclusive wins here. A device
            // whose protocol needs exclusive access is a documented gap
            // (see brands.ts), not something to solve by reintroducing the
            // freeze.
            //
            // `set_open_exclusive` only exists on Darwin (hidapi-rs gates it
            // behind `#[cfg(target_os = "macos")]` — CONFIRMED by reading
            // its own doc comment: "By default on Darwin platform all
            // devices ... are opened in exclusive mode"). Windows/Linux
            // don't share that exclusive-by-default behavior in the first
            // place, so there's nothing here to turn off on those
            // platforms.
            #[cfg(target_os = "macos")]
            api.set_open_exclusive(false);
            *guard = Some(api);
        }
    }
    f(guard.as_mut().expect("just initialized above"))
}

// hid_list_interfaces and hid_open stay synchronous (blocking the main
// thread) deliberately, unlike every other command below: they're the two
// that touch `HidApi` itself — `HidApi::new()`/`refresh_devices()`/
// `open_path()`, the calls that actually open a device handle through
// macOS's IOHIDManager, as opposed to reading/writing an already-open one.
// Making them `async fn` (moving that off the main thread, onto Tokio's
// runtime) reproduced a real hang requiring a force-quit — every prior
// successful connect this session happened with these on the main thread,
// and there is no equivalent evidence they're safe off it the way the
// reader threads' plain read()/write() on an open handle demonstrably are
// (those have run on a spawned thread since the very start of this file's
// history without incident). Left blocking here; the high-frequency calls
// during a `readStatus()` walk — the ones that actually caused the original
// UI freeze — are the ones below that stayed async.
#[tauri::command]
pub fn hid_list_interfaces(
    api_handle: tauri::State<HidApiHandle>,
    vendor_ids: Vec<u16>,
) -> Result<Vec<HidInterface>, String> {
    with_hid_api(&api_handle, |api| {
        let mut groups: HashMap<(u16, u16), Vec<&hidapi::DeviceInfo>> = HashMap::new();
        for info in api.device_list() {
            if !vendor_ids.contains(&info.vendor_id()) {
                continue;
            }
            applog!(
                "[hid] enumerate: vid={:04x} pid={:04x} path={} usage_page={:04x} usage={:04x} product={:?}",
                info.vendor_id(), info.product_id(),
                info.path().to_string_lossy(),
                info.usage_page(), info.usage(),
                info.product_string(),
            );
            groups
                .entry((info.vendor_id(), info.product_id()))
                .or_default()
                .push(info);
        }

        // On Windows, also enumerate device-interface paths via SetupDi*
        // that hidapi may have skipped (sub-collection paths with &colNN
        // in the device-id).  Record their paths so hid_open can open them.
        #[cfg(target_os = "windows")]
        {
            let mut extra_paths: HashMap<(u16, u16), Vec<String>> = HashMap::new();
            for &vid in &vendor_ids {
                let pids: Vec<u16> = groups
                    .keys()
                    .filter(|(v, _)| *v == vid)
                    .map(|(_, p)| *p)
                    .collect();
                for &pid in &pids {
                    for path in windows_hid_enumerate::enumerate_paths(vid, pid) {
                        applog!("[hid] windows enumerate: vid={vid:04x} pid={pid:04x} path={path}");
                        extra_paths.entry((vid, pid)).or_default().push(path);
                    }
                }
            }
            // Log paths that Windows found but hidapi missed
            for ((vid, pid), paths) in &extra_paths {
                let existing: HashSet<String> = groups
                    .get(&(*vid, *pid))
                    .map(|v| {
                        v.iter()
                            .map(|i| i.path().to_string_lossy().into_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                for path in paths {
                    if !existing.contains(path.as_str()) {
                        applog!(
                            "[hid] windows extra path for {vid:04x}:{pid:04x}: {path}"
                        );
                    }
                }
            }
            // Store extra paths in a global so hid_open can retrieve them
            EXTRA_WINDOWS_PATHS.lock().clear();
            for ((vid, pid), paths) in extra_paths {
                EXTRA_WINDOWS_PATHS
                    .lock()
                    .insert((vid, pid), paths);
            }
        }

        let mut result: Vec<HidInterface> = groups
            .into_iter()
            .map(|((vendor_id, product_id), infos)| {
                let product_string = infos
                    .iter()
                    .find_map(|info| info.product_string())
                    .unwrap_or("")
                    .to_string();
                let manufacturer_string = infos
                    .iter()
                    .find_map(|info| info.manufacturer_string())
                    .unwrap_or("")
                    .to_string();
                HidInterface {
                    key: interface_key(vendor_id, product_id),
                    vendor_id,
                    product_id,
                    product_string,
                    manufacturer_string,
                    collections: interface_collections(api, &infos),
                }
            })
            .collect();
        result.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(result)
    })
}

// See the comment on hid_list_interfaces above — kept synchronous for the
// same reason.
#[tauri::command]
pub fn hid_open(
    app: tauri::AppHandle,
    api_handle: tauri::State<HidApiHandle>,
    registry: tauri::State<HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    preferred_usage_page: Option<u16>,
    preferred_usage: Option<u16>,
) -> Result<(), String> {
    let group_key = (vendor_id, product_id);
    applog!("[hid] hid_open start {vendor_id:04x}:{product_id:04x}");
    {
        let mut map = registry.0.lock();
        if let Some(existing) = map.get(&group_key) {
            // Held for the session now (see `hid_close`), so a group is only
            // still valid while its reader threads are: they end when the
            // device goes away, which is exactly when the handles behind them
            // have to be replaced rather than handed back.
            //
            // `any`, not `all`: on a composite device some interfaces never
            // deliver input at all, so their readers end immediately and
            // legitimately. Only every reader being finished means the device
            // itself is gone — treating partial death as staleness reopened the
            // device on nearly every request, which is the churn holding the
            // link is meant to remove.
            if existing.readers.iter().any(|reader| !reader.is_finished()) {
                applog!("[hid] hid_open: already open and alive, returning");
                return Ok(());
            }
            applog!("[hid] hid_open: previous handles are stale, reopening");
            if let Some(stale) = map.remove(&group_key) {
                drop(map);
                for split in &stale.splits {
                    split.stop.store(true, Ordering::Relaxed);
                }
                for reader in stale.readers {
                    let _ = reader.join();
                }
            }
        }
    }

    let devices = with_hid_api(&api_handle, |api| {
        // `mut` looks unnecessary from a macOS build's perspective —
        // pushed into only in the #[cfg(target_os = "windows")] block
        // below, so Windows genuinely needs it mutable even though macOS
        // can't see why.
        #[cfg_attr(not(target_os = "windows"), allow(unused_mut))]
        let mut infos: Vec<_> = api
            .device_list()
            .filter(|info| {
                info.vendor_id() == vendor_id
                    && info.product_id() == product_id
            })
            .collect();
        // Split order decides where `try_each` sends a request first, and a
        // split that accepts a write but never answers pins the route cache to
        // itself — every request then reads back an all-zero packet.
        //
        // A brand whose control channel is a *standard* collection names it
        // outright and it goes first: Razer is the case in point, whose driver
        // documents the channel as "the interface whose only collection is
        // Generic Desktop Mouse" (usage page 0x0001, usage 0x0002). The
        // heuristics below are the opposite of what it needs — 0x000C is
        // ranked early for the vendor case, and on the attached DeathAdder V3
        // HyperSpeed the consumer collection is the *first* split, which is
        // exactly the all-zero reply the app was reporting ("Class 0x00
        // command 0x81 returned status 0x00") while a direct probe of the
        // mouse collection answered properly.
        //
        // Everything else keeps the old ordering: Vendor-Specific collections
        // (>= 0xFF00), then Consumer Control (0x000C). On Linux, hidapi can
        // open the standard mouse pointer collection (0x0001), which often
        // acts as a black hole: it successfully accepts feature reports but
        // drops them or returns zeroes — which is why vendor collections are
        // tried first for the brands that don't declare one.
        let ranked = |page: u16, usage: u16| match (preferred_usage_page, preferred_usage) {
            (Some(preferred_page), Some(preferred)) if page == preferred_page && usage == preferred => 0,
            _ if page >= 0xFF00 => 1,
            _ if page == 0x000C => 2,
            _ => 3,
        };
        infos.sort_by_key(|info| ranked(info.usage_page(), info.usage()));
        #[cfg_attr(not(target_os = "windows"), allow(unused_mut))]
        let mut paths: Vec<String> = Vec::new();
        // One handle per *interface path* is what a split set means: the point
        // of a split is a different interface to try a request on. hidapi
        // lists one entry per usage on an interface, so one physical interface
        // shows up several times, and opening every entry gave this app
        // several IOHIDDevice handles — each with its own reader thread
        // hammering `hid_read` — onto the same device. CONFIRMED in the
        // on-disk log for the attached Razer: six opens of
        // DevSrvsID:4295095113 across usage pages 0001/000c/0001/0080, out of
        // nine splits in total, where the device really has three interfaces.
        // `infos` is already sorted by rank, so the first entry for a path is
        // the collection this brand actually wants (the mouse collection for
        // Razer).
        let mut seen_paths: HashSet<String> = HashSet::new();
        for info in infos {
            let path = info.path().to_string_lossy().into_owned();
            let (usage_page, usage) = (info.usage_page(), info.usage());
            if !seen_paths.insert(path.clone()) {
                applog!(
                    "[hid] hid_open: same interface listed again as usage_page={usage_page:04x} usage={usage:04x}, not opened twice: {path}"
                );
                continue;
            }
            applog!(
                "[hid] hid_open: hidapi path for {vendor_id:04x}:{product_id:04x}: {path} usage_page={usage_page:04x} usage={usage:04x}"
            );
            paths.push(path);
        }


        // On Windows, merge in any extra sub-collection paths that
        // hidapi missed but SetupDi enumerated.
        #[cfg(target_os = "windows")]
        {
            if let Some(extra) = EXTRA_WINDOWS_PATHS.lock().remove(&(vendor_id, product_id))
            {
                for path in &extra {
                    if !paths.iter().any(|p| p == path) {
                        applog!(
                            "[hid] hid_open: extra windows path for {vendor_id:04x}:{product_id:04x}: {path}"
                        );
                        paths.push(path.clone());
                    }
                }
            }
        }
        if paths.is_empty() {
            return Err("No matching HID interface is currently connected.".into());
        }
        // Best-effort per path: a device with several top-level collections
        // can have some open fine and others rejected for reasons that have
        // nothing to do with the collection this brand's driver actually
        // needs (a boot mouse/keyboard collection gated by macOS's Input
        // Monitoring permission, for instance, sitting alongside the vendor
        // collection a driver here actually talks to). Failing the whole
        // group over one inaccessible split would block drivers that never
        // needed that split in the first place. Only error out if literally
        // none of them opened.
        let mut opened = Vec::new();
        let mut last_error = None;
        for path in &paths {
            let c_path = match CString::new(path.as_str()) {
                Ok(c) => c,
                Err(_) => continue,
            };
            match api.open_path(&c_path) {
                Ok(device) => {
                    applog!("[hid] hid_open: successfully opened: {path}");
                    opened.push(device);
                }
                Err(error) => {
                    applog!("[hid] hid_open: failed to open {path}: {error}");
                    last_error = Some(error.to_string());
                }
            }
        }
        if opened.is_empty() {
            return Err(last_error.unwrap_or_else(|| "No HID interface could be opened.".into()));
        }
        applog!("[hid] hid_open: opened {} split(s)", opened.len());
        Ok(opened)
    })?;

    let key = interface_key(vendor_id, product_id);
    // Shared across every split's reader thread for this group — see
    // `spawn_reader`'s own docs for why this exists.
    let last_reply = Arc::new(Mutex::new(None));
    let mut splits = Vec::new();
    let mut readers = Vec::new();
    for (index, device) in devices.into_iter().enumerate() {
        let stop = Arc::new(AtomicBool::new(false));
        let split = Arc::new(OpenSplit {
            device: Mutex::new(device),
            stop: stop.clone(),
        });
        readers.push(spawn_reader(
            app.clone(),
            key.clone(),
            index,
            split.clone(),
            last_reply.clone(),
        ));
        splits.push(split);
    }

    registry.0.lock().insert(
        group_key,
        OpenGroup {
            splits,
            readers,
            routes: Mutex::new(HashMap::new()),
            silent: Mutex::new(HashSet::new()),
            last_sent: Mutex::new(HashMap::new()),
        },
    );
    applog!("[hid] hid_open: done, registered group");
    Ok(())
}

/// Standard HID++ short/long report ids (0x10/0x11) — see `brands.ts`'s own
/// list: nearly every driver in this app that answers a request/response
/// exchange through an input report (not a plain feature report) uses these
/// two, Logitech's HID++ included.
const REQUEST_REPLY_REPORT_IDS: [u8; 2] = [0x10, 0x11];

/// How recent a byte-identical prior reply has to be to count as the SAME
/// physical report delivered twice (macOS handing it to every open split's
/// handle) rather than a coincidentally-identical reply to a later, distinct
/// request.
const DUPLICATE_DELIVERY_WINDOW: Duration = Duration::from_millis(50);

/// The last request/reply-shaped report any split's reader delivered, and
/// which split delivered it.
///
/// The split is the part that makes duplicate suppression correct, and it was
/// missing: content alone cannot tell "macOS handed this one physical report
/// to all nine handles" from "the device sent the same bytes twice, in reply
/// to two different requests". `IRoot.getFeature()` replies carry no trace of
/// what was asked, so a mouse that lacks two features in a row (a PRO X
/// Superlight's 0x1000 and 0x1001 battery features, both absent — it reports
/// through 0x1004) answers both with the same all-zero "not found" payload.
/// MEASURED on real hardware: those two replies land ~9 ms apart, well inside
/// the window, so the second was swallowed as a duplicate delivery and
/// `readStatus()` sat waiting out its 6 s request timeout — "The mouse did not
/// answer. Move it or click a button." on every single connect, reproduced 3/3
/// times, with the walk dying at request 8 of 37 every time. The same walk
/// completed before the reader/writer mutex fix above only because requests
/// were ~200 ms apart then, i.e. outside this window — the slow path was
/// hiding it.
///
/// A report's own handle always delivers it exactly once, so an identical
/// report from the SAME split is a real second reply; only a different split
/// can be macOS re-delivering one report to another handle.
struct LastReply {
    report_id: u8,
    data: Vec<u8>,
    seen_at: Instant,
    split: usize,
}

/// HID++ 2.0 error codes (byte 4 of a 0xFF error reply) — mirrors
/// `HIDPP20_ERRORS` in mouse-protocol's `logitech/index.ts`, purely so a
/// rejection is legible in these logs without needing the webview's own
/// devtools console open.
fn hidpp20_error_name(code: u8) -> Option<&'static str> {
    Some(match code {
        0x01 => "unknown request",
        0x02 => "invalid argument",
        0x03 => "value out of range",
        0x04 => "hardware error",
        0x05 => "Logitech internal error",
        0x06 => "invalid feature index",
        0x07 => "invalid function",
        0x08 => "device busy",
        0x09 => "unsupported",
        _ => return None,
    })
}

/// HID++ 1.0 error codes (byte 4 of a 0x8F error reply) — mirrors
/// `HIDPP10_ERRORS` in the same file.
fn hidpp10_error_name(code: u8) -> Option<&'static str> {
    Some(match code {
        0x01 => "invalid command",
        0x02 => "invalid address",
        0x03 => "invalid value",
        0x04 => "connection request failed",
        0x05 => "too many devices",
        0x06 => "already exists",
        0x07 => "device busy",
        0x08 => "unknown device",
        0x09 => "resource error",
        0x0a => "request unavailable",
        0x0b => "unsupported parameter value",
        0x0c => "wrong PIN code",
        _ => return None,
    })
}

/// Decodes an outgoing HID++ short/long request's own header — deviceIndex,
/// featureIndex, function id (the software id in the low nibble stripped
/// off), and parameter bytes — so a request can be matched by eye against
/// the reply (or rejection) it produced a few lines later in the log.
fn describe_hidpp_request(data: &[u8]) -> String {
    if data.len() < 3 {
        return format!("raw={data:02x?}");
    }
    let device_index = data[0];
    let feature_index = data[1];
    let function_id = data[2] >> 4;
    let params = &data[3..];
    format!(
        "hidpp deviceIndex=0x{device_index:02x} featureIndex=0x{feature_index:02x} \
         functionId=0x{function_id:02x} params={params:02x?}"
    )
}

/// Decodes an incoming report as a HID++ error notification, if it is one —
/// `[deviceIndex, 0xFF|0x8F, featureIndex, function+softwareId, errorCode]`,
/// mirroring `hidppErrorForRequest` in mouse-protocol's own driver. Returns
/// `None` for anything else (a normal reply, or a raw input/status report).
fn describe_hidpp_error(report_id: u8, data: &[u8]) -> Option<String> {
    if report_id != 0x10 && report_id != 0x11 {
        return None;
    }
    // `data` is the payload after the leading report id byte, same
    // alignment as mouse-protocol's own `report` array (WebHID strips the
    // report id out separately too, into `event.reportId`): data[0] is
    // deviceIndex, data[1] is the 0xFF/0x8F error marker, data[2] is the
    // echoed featureIndex from the rejected request, data[3] its echoed
    // function+softwareId byte, data[4] the error code.
    let marker = *data.get(1)?;
    let feature_index = *data.get(2)?;
    let function_byte = *data.get(3)?;
    let error_code = *data.get(4).unwrap_or(&0);
    let (kind, name) = match marker {
        0xff => ("HID++ 2.0", hidpp20_error_name(error_code)),
        0x8f => ("HID++ 1.0", hidpp10_error_name(error_code)),
        _ => return None,
    };
    let reason = name.unwrap_or("unknown error");
    Some(format!(
        "{kind} ERROR: featureIndex=0x{feature_index:02x} functionByte=0x{function_byte:02x} \
         code=0x{error_code:02x} ({reason})"
    ))
}

fn spawn_reader(
    app: tauri::AppHandle,
    key: String,
    split_index: usize,
    split: Arc<OpenSplit>,
    last_reply: Arc<Mutex<Option<LastReply>>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0u8; 64];
        while !split.stop.load(Ordering::Relaxed) {
            // try_lock, not lock: a query/response exchange (hid_send_report
            // / hid_get_feature_report, called from the JS driver's own
            // readStatus()) needs this same device's lock and must never
            // wait behind this loop's read — reads are continuous and
            // best-effort (missing one poll cycle is fine), but a stuck
            // query is what "Connecting…" hanging looked like. If a writer
            // currently holds the lock, skip this cycle instead of blocking
            // for it.
            //
            // CONFIRMED on real hardware, twice over. First: this alone
            // isn't enough, because read_timeout() blocks for the full
            // READ_POLL_TIMEOUT_MS while STILL HOLDING the try_lock guard,
            // and this loop went straight back into try_lock() the instant
            // that guard dropped, with nothing in between. A writer parked
            // on the blocking `.lock()` in try_each has no fairness
            // guarantee against that: macOS's pthread mutex doesn't queue a
            // parked waiter ahead of a thread that keeps re-acquiring via
            // try_lock, so this loop could win every single re-lock race
            // indefinitely — a real livelock, not a slow poll. Seen live:
            // resolveDeviceIndex walking receiver pairing slots got every
            // reply promptly right up until the write for deviceIndex=0xff,
            // whose try_each sat on "waiting for lock…" and never
            // progressed — this loop had just finished draining a backlog
            // and immediately re-entered another 200ms held read_timeout,
            // over and over, never leaving a gap for the scheduler to hand
            // the lock to the parked writer. The gap sleep below (outside
            // the lock, every iteration) is what creates that handover.
            //
            // Second, the other half of the same coin: because the writer
            // could only ever win the lock during that gap, the *length* of
            // READ_POLL_TIMEOUT_MS was the latency of every single request
            // in a walk. That is why it is 10ms now, not 200ms — see its own
            // docs for the measurements.
            let read = match split.device.try_lock() {
                Some(device) => device.read_timeout(&mut buffer, READ_POLL_TIMEOUT_MS),
                None => {
                    thread::sleep(Duration::from_millis(READ_IDLE_GAP_MS));
                    continue;
                }
            };
            match read {
                Ok(0) => {}
                Ok(length) => {
                    // Some backends prefix numbered reports with the report
                    // id, some don't — assume they do (report id in
                    // buffer[0]), matching the equivalent ambiguity the
                    // Node adapter and the Rust Pulsar driver both handle
                    // the same way. Drivers here match responses by their
                    // own payload content, not strictly by report id.
                    let report_id = buffer[0];
                    let data = buffer[1..length].to_vec();
                    // CONFIRMED on real hardware: macOS delivers the same
                    // physical report to every open split's handle, not just
                    // the one collection that actually declares it — every
                    // request/reply exchange gets emitted here once per
                    // split, not once. A driver's own reply matching keys on
                    // (deviceIndex, featureIndex, functionId), not report
                    // content, and several unrelated requests can share that
                    // key (every HID++ getFeature() call does) — so a
                    // duplicate arriving late enough to land after the next
                    // request went out gets consumed as that request's
                    // answer instead, silently corrupting whichever field
                    // was being read at the time. WebHID never has this
                    // problem: one collection is one `HIDDevice`, so a
                    // report is only ever delivered to the listener for the
                    // collection that actually declared it. Collapsing an
                    // exact repeat of the immediately preceding
                    // request/reply-shaped report is the equivalent fix
                    // here. Left unrestricted to every report id — a raw
                    // input stream (movement, clicks) can legitimately
                    // repeat the same idle payload many times in a row, and
                    // collapsing those would silently drop real reports.
                    //
                    // CONFIRMED on real hardware this needs a time window too,
                    // not a bare content match: an IRoot.getFeature() "not
                    // found" reply carries no trace of which feature id was
                    // asked (deviceIndex, echoed featureIndex=0, echoed
                    // functionId, all-zero payload) — so two DIFFERENT,
                    // genuinely sequential getFeature() calls that both come
                    // back "not found" are byte-for-byte identical. A bare
                    // content-match dedup silently ate the second one
                    // (getFeature(0x1001) after getFeature(0x1000), both
                    // absent on this mouse) and readStatus() hung waiting
                    // for a reply that was dropped, not missing.
                    //
                    // A time window alone turned out NOT to be enough either,
                    // and that is the state this was in: a healthy link
                    // answers a short HID++ request in ~9 ms (measured, see
                    // LastReply), so two identical replies to two consecutive
                    // requests land *inside* any window wide enough to catch
                    // macOS's duplicate deliveries — and swallowing the
                    // second one fails the whole walk. `LastReply`'s split is
                    // what actually separates the two cases: one physical
                    // report is delivered to every split's handle exactly
                    // once each, so identical bytes from a *different* split
                    // are that duplication, while identical bytes from the
                    // same split are a new, real reply.
                    let mut last = last_reply.lock();
                    let is_duplicate = REQUEST_REPLY_REPORT_IDS.contains(&report_id)
                        && matches!(
                            last.as_ref(),
                            Some(previous)
                                if previous.split != split_index
                                    && previous.report_id == report_id
                                    && previous.data == data
                                    && previous.seen_at.elapsed() < DUPLICATE_DELIVERY_WINDOW
                        );
                    if is_duplicate {
                        continue;
                    }
                    if REQUEST_REPLY_REPORT_IDS.contains(&report_id) {
                        *last = Some(LastReply {
                            report_id,
                            data: data.clone(),
                            seen_at: Instant::now(),
                            split: split_index,
                        });
                    }
                    drop(last);
                    // Only log HID++ protocol traffic (report ids 0x10/0x11),
                    // and only once it's cleared the dedup check above — not
                    // every raw report a split hands us. The continuous
                    // stream of movement/click/battery-poll reports on other
                    // report ids is real and gets emitted below same as
                    // always, it's just not useful to print: at full poll
                    // rate it drowns the handful of request/reply lines that
                    // actually matter for debugging a connect in thousands of
                    // irrelevant ones.
                    if REQUEST_REPLY_REPORT_IDS.contains(&report_id) {
                        applog!(
                            "[hid] reader {key}: got {length} bytes, reportId=0x{report_id:02x} data={data:02x?}"
                        );
                        if let Some(decoded) = describe_hidpp_error(report_id, &data) {
                            applog!("[hid] reader {key}: *** {decoded} ***");
                        }
                    }
                    let _ = app.emit(
                        "hid-input-report",
                        HidInputReportPayload {
                            key: key.clone(),
                            report_id,
                            data,
                        },
                    );
                }
                Err(error) => {
                    // A transient read error (e.g. device unplugged) — the
                    // stop flag (set by hid_close) is what ends this loop
                    // deliberately; anything else just backs off briefly.
                    applog!("[hid] reader {key}: read_timeout error: {error}");
                    thread::sleep(Duration::from_millis(50));
                }
            }
            // Deliberately after the report above has been emitted and with
            // no lock held — see READ_IDLE_GAP_MS.
            thread::sleep(Duration::from_millis(READ_IDLE_GAP_MS));
        }
    })
}

#[tauri::command]
pub async fn hid_close(
    _registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
) -> Result<(), String> {
    // Deliberately does NOT release anything. Every protocol driver calls
    // `close()` at the end of a walk, and honouring it meant the app closed
    // and reopened the device on every status read: a full enumeration plus
    // one open per split, and — worse — a fresh `OpenGroup`, throwing away
    // what the app had learned about which split actually answers (see
    // `OpenGroup::silent` and `last_sent`). The handles, the reader threads and
    // that knowledge now live until the process exits. `hid_open` still
    // replaces a group whose interfaces have gone, so unplugging is handled.
    applog!("[hid] hid_close {vendor_id:04x}:{product_id:04x}: kept open for the session");
    Ok(())
}

#[tauri::command]
pub async fn hid_send_report(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    data: Vec<u8>,
) -> Result<(), String> {
    applog!(
        "[hid] hid_send_report {vendor_id:04x}:{product_id:04x} reportId=0x{report_id:02x} {}",
        describe_hidpp_request(&data),
    );
    let result = with_open_group(&registry, vendor_id, product_id, |group| {
        let mut frame = Vec::with_capacity(data.len() + 1);
        frame.push(report_id);
        frame.extend_from_slice(&data);
        send_each(&group.splits, &frame, false, vendor_id == RAZER_VENDOR_ID)
    });
    if result.is_err() {
        drop_group(&registry, vendor_id, product_id);
    }
    applog!("[hid] hid_send_report done: {result:?}");
    result
}

#[tauri::command]
pub async fn hid_send_feature_report(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    data: Vec<u8>,
) -> Result<(), String> {
    applog!(
        "[hid] hid_send_feature_report {vendor_id:04x}:{product_id:04x} reportId=0x{report_id:02x} {}",
        describe_hidpp_request(&data),
    );
    let result = with_open_group(&registry, vendor_id, product_id, |group| {
        let mut frame = Vec::with_capacity(data.len() + 1);
        frame.push(report_id);
        frame.extend_from_slice(&data);
        // Remembered so a read that echoes these bytes back is recognised as
        // silence rather than an answer — see `is_unprocessed_echo`.
        group.last_sent.lock().insert(report_id, data.clone());
        // Fan out for Razer only: see `send_each`.
        send_each(&group.splits, &frame, true, vendor_id == RAZER_VENDOR_ID)
    });
    if result.is_err() {
        drop_group(&registry, vendor_id, product_id);
    }
    applog!("[hid] hid_send_feature_report done: {result:?}");
    result
}

#[tauri::command]
pub async fn hid_get_input_report(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    length: usize,
) -> Result<Vec<u8>, String> {
    applog!("[hid] hid_get_input_report {vendor_id:04x}:{product_id:04x} reportId=0x{report_id:02x} length={length}");
    let result = with_open_group(&registry, vendor_id, product_id, |group| {
        let mut result: Option<Vec<u8>> = None;
        let outcome = try_each(&group.splits, &group.routes, &group.silent, report_id, |device| {
            let mut buffer = vec![0u8; length + 1];
            buffer[0] = report_id;
            let read = device
                .get_input_report(&mut buffer)
                .map_err(|error| error.to_string())?;
            let data_end = read.min(buffer.len());
            result = Some(buffer[1..data_end].to_vec());
            // Input reports are noise-prone by nature — a mouse's idle report
            // really is zeros — so an empty one is not treated as silence.
            Ok(SplitAnswer::Answered)
        });
        outcome.and(result.ok_or_else(|| "no split returned data".to_string()))
    });
    if result.is_err() {
        drop_group(&registry, vendor_id, product_id);
    }
    applog!("[hid] hid_get_input_report done: {result:?}");
    result
}
#[tauri::command]
pub async fn hid_get_feature_report(
    registry: tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    report_id: u8,
    length: usize,
) -> Result<Vec<u8>, String> {
    applog!("[hid] hid_get_feature_report {vendor_id:04x}:{product_id:04x} reportId=0x{report_id:02x} length={length}");
    let result = with_open_group(&registry, vendor_id, product_id, |group| {
        let mut result: Option<Vec<u8>> = None;
        let outcome = try_each(&group.splits, &group.routes, &group.silent, report_id, |device| {
            let mut buffer = vec![0u8; length + 1];
            buffer[0] = report_id;
            let read = device
                .get_feature_report(&mut buffer)
                .map_err(|error| error.to_string())?;
            // hidapi's get_feature_report puts the report ID at buf[0] and
            // data at buf[1..].  On all platforms the return value includes
            // the report-ID byte (Linux ioctl HIDIOCGFEATURE returns total
            // bytes placed in the buffer; Windows returns the same).
            // WebHID's receiveFeatureReport strips the report ID, so we
            // must strip it here to match the browser API contract.
            let data_end = read.min(buffer.len());
            let data = buffer[1..data_end].to_vec();
            // An interface that accepted the request but does not carry the
            // device's traffic either answers with an all-zero packet or echoes
            // the request straight back. CONFIRMED for this app's own Razer:
            // with two mouse-collection splits on the receiver, the first by
            // rank replies to every command with our own bytes and status 0x00
            // ("Class 0x04 command 0x05 returned status 0x00") while the mouse
            // is working fine — the desktop's cursor proves it, because the
            // desktop is served by the other interface. Counting that as an
            // answer is what pinned `routes` to the dead interface and made the
            // device look asleep. Report it as silence so `try_each` moves on
            // and never caches this split for the id.
            let echoed = {
                let sent = group.last_sent.lock();
                is_unprocessed_echo(&data, sent.get(&report_id))
            };
            if echoed || data.iter().all(|byte| *byte == 0) {
                result = Some(data);
                return Ok(SplitAnswer::Silent);
            }
            result = Some(data);
            Ok(SplitAnswer::Answered)
        });
        // Every split silent: hand the empty packet back rather than inventing
        // an error the protocol layer cannot describe (a genuinely dead device
        // should produce its own status, as it did before this check existed).
        match outcome {
            Ok(()) => result.ok_or_else(|| "no split returned data".to_string()),
            Err(error) => match result {
                Some(data) => Ok(data),
                None => Err(error),
            },
        }
    });
    if result.is_err() {
        drop_group(&registry, vendor_id, product_id);
    }
    applog!("[hid] hid_get_feature_report done: {result:?}");
    result
}

/// Runs `operation` against the open splits for a group (plus its
/// report-id→split route cache — see `OpenGroup::routes`), trying every
/// split in turn on a cache miss — the report id in question may only be
/// declared on one of them, same reasoning as the Node adapter's
/// `sendReport`/`receiveFeatureReport`.
fn with_open_group<T>(
    registry: &tauri::State<'_, HidRegistry>,
    vendor_id: u16,
    product_id: u16,
    operation: impl FnOnce(&OpenGroup) -> Result<T, String>,
) -> Result<T, String> {
    let map = registry.0.lock();
    let group = map
        .get(&(vendor_id, product_id))
        .ok_or_else(|| "This HID interface is not open.".to_string())?;
    operation(group)
}

/// Sends one report to the group's interfaces.
///
/// With `fan_out` — which is Razer only, mirroring OpenMouse-Bridge: its
/// `send_report` routes by *declared report ids* wherever a report descriptor
/// parses, and only inspects nothing for Razer, fanning the command out to
/// every interface instead. Which of a composite receiver's interfaces reaches
/// the mouse is not knowable from the outside, so the command is written to all
/// of them and whichever one is live processes it. Writing only to the first
/// interface that merely *accepts* the write — which is what this app did —
/// leaves the command on an interface that never forwards it, and the read that
/// follows finds no answer at all.
///
/// Every other brand keeps first-accept-wins: their interfaces are the ones the
/// protocol can route, and the Logitech path in particular is built on a
/// HID++ control interface that should not be written to through its sibling
/// collections.
///
/// (The general form of the bridge's rule — parse each interface's report
/// descriptor and route by what it declares — is the right eventual shape for
/// this app too; it needs descriptor parsing this file does not have yet.)
fn send_each(
    splits: &[Arc<OpenSplit>],
    frame: &[u8],
    feature: bool,
    fan_out: bool,
) -> Result<(), String> {
    let mut sent = false;
    let mut last_error = None;
    for (index, split) in splits.iter().enumerate() {
        applog!(
            "[hid] send_each: split {index}/{} waiting for lock…",
            splits.len()
        );
        let device = split.device.lock();
        let outcome = if feature {
            device.send_feature_report(frame)
        } else {
            device.write(frame).map(|_| ())
        };
        match outcome {
            Ok(_) => {
                sent = true;
                if !fan_out {
                    return Ok(());
                }
            }
            Err(error) => last_error = Some(error.to_string()),
        }
    }
    if sent {
        Ok(())
    } else {
        Err(last_error.unwrap_or_else(|| "No HID interface accepted the request.".to_string()))
    }
}

/// Drops a group whose handles no longer work, so the next call reopens the
/// device from a fresh enumeration.
///
/// `try_each` only gives up when *every* split has failed at the HID level —
/// "hidapi error: Device is disconnected" after a receiver is unplugged, which
/// is exactly what the app hit once it started holding handles for the session
/// (see `hid_close`): the readers stay blocked in `hid_read` on a vanished
/// device rather than finishing, so reader liveness cannot be the staleness
/// test, and without this the dead handles would be handed back forever.
fn drop_group(registry: &tauri::State<'_, HidRegistry>, vendor_id: u16, product_id: u16) {
    if let Some(group) = registry.0.lock().remove(&(vendor_id, product_id)) {
        applog!("[hid] dropped the handles for {vendor_id:04x}:{product_id:04x}: they no longer work");
        for split in &group.splits {
            split.stop.store(true, Ordering::Relaxed);
        }
        // Readers are deliberately not joined: they are the ones blocked in
        // `hid_read` on the vanished device, and the OS returns them on its own
        // schedule. Joining would block this command on exactly that.
    }
}

/// What one split did with a request. `Silent` is this app's own term for an
/// interface that accepted it and answered with an empty packet — a receiver
/// interface that is up but not carrying the mouse's traffic. See
/// `hid_get_feature_report`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitAnswer {
    Answered,
    Silent,
}

/// True when a reply is byte-for-byte the request that preceded it: the
/// interface echoed it back instead of answering. CONFIRMED for this app's own
/// Razer — with two mouse-collection splits on one receiver, the first by rank
/// replies to every command with our own bytes and status 0x00
/// ("Class 0x04 command 0x05 returned status 0x00") while the mouse works fine
/// on the desktop, because the desktop is served by the other interface.
/// Counting that as an answer is what pinned `routes` to the dead one. An
/// honest reply carries something the request did not: a status, a value, an
/// error code.
fn is_unprocessed_echo(reply: &[u8], last_sent: Option<&Vec<u8>>) -> bool {
    match last_sent {
        Some(sent) if !sent.is_empty() && sent.len() == reply.len() => sent.as_slice() == reply,
        _ => false,
    }
}

fn try_each(
    splits: &[Arc<OpenSplit>],
    routes: &Mutex<HashMap<u8, usize>>,
    silent: &Mutex<HashSet<usize>>,
    report_id: u8,
    mut operation: impl FnMut(&hidapi::HidDevice) -> Result<SplitAnswer, String>,
) -> Result<(), String> {
    // Try whichever split answered this exact report id last time first —
    // see `OpenGroup::routes`. Falls through to the rest in order on a miss
    // (first call for this report id, or the routed split stopped working),
    // so this is a pure optimization, never a correctness change.
    let hinted = routes.lock().get(&report_id).copied();
    let known_silent = silent.lock().clone();
    // Splits already known to answer with an empty packet go last — but never
    // dropped entirely: a device that has since been replugged deserves
    // another chance, so when every split is marked silent the slate is wiped
    // instead of leaving nothing to try.
    let candidates: Vec<usize> = (0..splits.len())
        .filter(|index| !known_silent.contains(index))
        .collect();
    let order: Vec<usize> = if candidates.is_empty() {
        silent.lock().clear();
        (0..splits.len()).collect()
    } else {
        match hinted {
            Some(hint) if candidates.contains(&hint) => std::iter::once(hint)
                .chain(candidates.iter().copied().filter(|index| *index != hint))
                .collect(),
            _ => candidates,
        }
    };

    let mut last_error = None;
    for index in order {
        let split = &splits[index];
        applog!(
            "[hid] try_each: split {index}/{} waiting for lock…",
            splits.len()
        );
        let device = split.device.lock();
        applog!("[hid] try_each: split {index} got lock, calling operation…");
        let outcome = operation(&device);
        applog!("[hid] try_each: split {index} operation returned: {outcome:?}");
        match outcome {
            Ok(SplitAnswer::Answered) => {
                routes.lock().insert(report_id, index);
                return Ok(());
            }
            Ok(SplitAnswer::Silent) => {
                // Never cached as this report id's route, which is the whole
                // point: a silent split used to look like the winner.
                applog!(
                    "[hid] try_each: split {index}/{} answered with an empty packet, not using it",
                    splits.len()
                );
                let mut marked = silent.lock();
                marked.insert(index);
                if marked.len() >= splits.len() {
                    marked.clear();
                }
            }
            Err(error) => last_error = Some(error.to_string()),
        }
    }
    Err(last_error.unwrap_or_else(|| "No HID interface accepted the request.".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// `with_hid_api` must *refuse* a call that arrives while another one
    /// holds the handle, never wait for it. Waiting is what froze the app:
    /// hidapi's macOS enumerator pumps the calling thread's run loop
    /// (`process_pending_events`), so an `invoke` serviced by that pump
    /// re-enters here on the same thread that already holds the guard — a
    /// `lock()` there parks forever (`__psynch_cvwait` under
    /// `hid_list_interfaces`, nested inside `hid_open`'s `hid_enumerate`,
    /// observed with `sample`) and the window stops responding while the
    /// process stays alive. Run the second caller on its own thread so a
    /// regression to a blocking lock fails this test instead of hanging it,
    /// and so nothing here touches hidapi off the main thread.
    #[test]
    fn a_contended_call_is_refused_instead_of_waiting() {
        let handle = Arc::new(HidApiHandle::default());
        let guard = handle.0.try_lock().expect("a fresh handle is unlocked");
        let (sender, receiver) = mpsc::channel();
        let nested = Arc::clone(&handle);
        std::thread::spawn(move || {
            let _ = sender.send(with_hid_api(&nested, |_| Ok(())));
        });
        let outcome = receiver.recv_timeout(Duration::from_secs(5));
        drop(guard);
        assert_eq!(
            outcome.expect("a contended call must return, not block"),
            Err(HID_BUSY.to_string()),
        );
    }

    /// The reply that made a working Razer look asleep was the request echoed
    /// straight back with status 0x00. Pin the rule that tells one from an
    /// answer: treating an echo as an answer is what pinned the route cache to
    /// a dead interface and made every connect fail until the user clicked
    /// again.
    #[test]
    fn a_reply_that_is_our_own_request_is_not_an_answer() {
        let request = vec![0x1f, 0x00, 0x00, 0x04, 0x05, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00];
        assert!(
            is_unprocessed_echo(&request, Some(&request)),
            "an echo of the request must not count as an answer",
        );
        let mut answered = request.clone();
        answered[5] = 0x02;
        assert!(!is_unprocessed_echo(&answered, Some(&request)));
        assert!(!is_unprocessed_echo(&request, None), "nothing sent, nothing to echo");
        assert!(!is_unprocessed_echo(&[], Some(&Vec::new())), "an empty request proves nothing");
    }
}
