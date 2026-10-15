//! Windows half of the game overlay (`configure_overlay` on macOS). GPUI already
//! creates a pop-up as a borderless, topmost tool window without a taskbar button
//! and opens it without activating it; what it does not do is let clicks through,
//! or put the pop-up on the monitor the game is on. This adds both.
//!
//! Raw Win32 (no `windows` crate dependency), kept self-contained so it can be
//! compiled against a Windows target from any host.

use super::{place_in_area, OverlayCorner};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Default)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    /// The monitor minus the taskbar and docked bars.
    work: Rect,
    flags: u32,
}

#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> isize;
    fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
    fn GetMonitorInfoW(monitor: isize, info: *mut MonitorInfo) -> i32;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: isize, index: i32, value: isize) -> isize;
    fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    fn SetLayeredWindowAttributes(hwnd: isize, key: u32, alpha: u8, flags: u32) -> i32;
}

#[link(name = "shcore")]
extern "system" {
    fn GetDpiForMonitor(monitor: isize, kind: i32, dpi_x: *mut u32, dpi_y: *mut u32) -> i32;
}

const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOPMOST: isize = 0x0000_0008;
const WS_EX_TRANSPARENT: isize = 0x0000_0020;
const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
const WS_EX_LAYERED: isize = 0x0008_0000;
const WS_EX_NOACTIVATE: isize = 0x0800_0000;
const HWND_TOPMOST: isize = -1;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_SHOWWINDOW: u32 = 0x0040;
const MONITOR_DEFAULTTONEAREST: u32 = 2;
const MDT_EFFECTIVE_DPI: i32 = 0;
const LWA_ALPHA: u32 = 0x2;

/// Makes the (already open) overlay window click-through and non-activating,
/// keeps it topmost, and moves it into `corner` of the work area of the monitor
/// the foreground window (the game) is on, sized for that monitor's scale.
///
/// `hwnd` is the pop-up's own window handle. `width`/`height`/`margin` are in
/// logical pixels.
pub fn configure_overlay_hwnd(hwnd: isize, width: f64, height: f64, corner: OverlayCorner, margin: f64) {
    unsafe {
        // Never let a click reach the overlay or activate it: layered + transparent
        // is what makes the mouse pass through to a window of another process.
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            style | WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        );
        // A layered window shows nothing until its opacity is set; fully opaque,
        // since the pill's own alpha comes from what it draws.
        SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);

        // The monitor with the game on it, else the one the overlay is on.
        let foreground = GetForegroundWindow();
        let anchor = if foreground != 0 && foreground != hwnd { foreground } else { hwnd };
        let monitor = MonitorFromWindow(anchor, MONITOR_DEFAULTTONEAREST);
        let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
        let have_area = monitor != 0 && GetMonitorInfoW(monitor, &mut info) != 0;

        let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
        if monitor != 0 {
            let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        }
        let scale = (dpi_x.max(1) as f64) / 96.0;
        let (w, h) = ((width * scale).round() as i32, (height * scale).round() as i32);

        let (x, y) = if have_area {
            let m = (margin * scale).round() as i32;
            let work = info.work;
            let left = matches!(corner, OverlayCorner::TopLeft | OverlayCorner::BottomLeft);
            let top = matches!(corner, OverlayCorner::TopLeft | OverlayCorner::TopRight);
            (place_in_area(work.left, work.right, w, m, left), place_in_area(work.top, work.bottom, h, m, top))
        } else {
            (0, 0)
        };
        let flags = SWP_NOACTIVATE | SWP_SHOWWINDOW;
        if have_area {
            SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, flags);
        }
    }
}
