//! Native window opacity only; backdrop blur is owned by GPUI.
//!
//! `WindowBackgroundAppearance::Blurred` installs an `NSVisualEffectView`
//! behind the transparent Metal scene. App surfaces supply their own alpha;
//! `set_window_opacity(1.0)` keeps text and controls at full compositor opacity.
//! AppKit chooses the material's blur radius; there is no public radius API.

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::configure_overlay_hwnd;

#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};

/// Sets whole-window compositor opacity, including text and controls.
///
/// Call on the AppKit main thread. Background translucency must instead be
/// painted into the GPUI scene, leaving this value at 1.0.
pub fn set_window_opacity(opacity: f32) {
    #[cfg(target_os = "macos")]
    {
        let opacity = opacity.clamp(0.4, 1.0) as f64;
        unsafe {
            let app: id = msg_send![class!(NSApplication), sharedApplication];
            let windows: id = msg_send![app, windows];
            let count: usize = msg_send![windows, count];
            if count == 0 {
                return;
            }
            // Resolve the live window on each call instead of caching an
            // unretained pointer that can outlive a closed window.
            let main: id = msg_send![app, mainWindow];
            let window: id = if main != nil {
                main
            } else if count == 1 {
                msg_send![windows, lastObject]
            } else {
                return;
            };
            if window != nil {
                let _: () = msg_send![window, setAlphaValue: opacity];
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = opacity;
    }
}

/// Which screen corner the game overlay sits in (same four as the Tauri app).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OverlayCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    #[default]
    BottomRight,
}

impl OverlayCorner {
    pub const ALL: [OverlayCorner; 4] = [
        OverlayCorner::TopLeft,
        OverlayCorner::TopRight,
        OverlayCorner::BottomLeft,
        OverlayCorner::BottomRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OverlayCorner::TopLeft => "Top left",
            OverlayCorner::TopRight => "Top right",
            OverlayCorner::BottomLeft => "Bottom left",
            OverlayCorner::BottomRight => "Bottom right",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            OverlayCorner::TopLeft => "top-left",
            OverlayCorner::TopRight => "top-right",
            OverlayCorner::BottomLeft => "bottom-left",
            OverlayCorner::BottomRight => "bottom-right",
        }
    }

    pub fn parse(text: &str) -> Self {
        match text {
            "top-left" => OverlayCorner::TopLeft,
            "top-right" => OverlayCorner::TopRight,
            "bottom-left" => OverlayCorner::BottomLeft,
            _ => OverlayCorner::BottomRight,
        }
    }
}

/// Where a corner placement lands along one axis: an `extent`-long box inside
/// `[start, end)`, `margin` from the nearer edge — at the start edge, or the end
/// edge. Pure integer maths in physical pixels, shared by the platform code and
/// testable anywhere (the work area can start at a negative coordinate on a
/// monitor left of or above the primary one).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn place_in_area(start: i32, end: i32, extent: i32, margin: i32, at_start: bool) -> i32 {
    if at_start {
        start + margin
    } else {
        end - extent - margin
    }
}

/// Turns the just-opened overlay window into a real overlay: placed in the
/// chosen corner of the *usable* area (visible frame, so never under the menu
/// bar or Dock) of the screen that has focus — where a game is — floating above
/// other apps' windows, click-through, and out of the Dock/Exposé/app switcher.
///
/// The window is found by its size. Call on the AppKit main thread right after
/// opening it. No-op off macOS (the pop-up kind is already topmost there, and
/// the caller positions it from the display's visible bounds).
pub fn configure_overlay(width: f64, height: f64, corner: OverlayCorner, margin: f64) {
    #[cfg(target_os = "macos")]
    {
        use cocoa::foundation::{NSPoint, NSRect, NSSize};
        unsafe {
            let app: id = msg_send![class!(NSApplication), sharedApplication];
            let windows: id = msg_send![app, windows];
            let count: usize = msg_send![windows, count];
            let screen: id = msg_send![class!(NSScreen), mainScreen];
            for i in 0..count {
                let window: id = msg_send![windows, objectAtIndex: i];
                let frame: NSRect = msg_send![window, frame];
                if (frame.size.width - width).abs() > 1.0 || (frame.size.height - height).abs() > 40.0 {
                    continue;
                }
                // GPUI opens even a "no titlebar" window as a titled one, and AppKit
                // draws that frame (rounded corners, an edge line, a shadow) around
                // the pill. Strip it to a plain borderless, clear window.
                let _: () = msg_send![window, setStyleMask: 0_usize];
                let clear: id = msg_send![class!(NSColor), clearColor];
                let _: () = msg_send![window, setBackgroundColor: clear];

                // Exactly the pill's size: removing the frame changes the window's.
                let size = NSSize::new(width, height);
                let origin = if screen != nil {
                    // AppKit's origin is bottom-left; the visible frame already
                    // excludes the menu bar and Dock.
                    let area: NSRect = msg_send![screen, visibleFrame];
                    let left = area.origin.x + margin;
                    let right = area.origin.x + area.size.width - width - margin;
                    let bottom = area.origin.y + margin;
                    let top = area.origin.y + area.size.height - height - margin;
                    let (x, y) = match corner {
                        OverlayCorner::TopLeft => (left, top),
                        OverlayCorner::TopRight => (right, top),
                        OverlayCorner::BottomLeft => (left, bottom),
                        OverlayCorner::BottomRight => (right, bottom),
                    };
                    NSPoint::new(x, y)
                } else {
                    frame.origin
                };
                let target = NSRect::new(origin, size);
                let _: () = msg_send![window, setFrame: target display: objc::runtime::YES];
                // NSScreenSaverWindowLevel: above fullscreen and borderless windows.
                let _: () = msg_send![window, setLevel: 1000_i64];
                let _: () = msg_send![window, setIgnoresMouseEvents: objc::runtime::YES];
                let _: () = msg_send![window, setHasShadow: objc::runtime::NO];
                // AppKit caches the shadow it computed for the old frame.
                let _: () = msg_send![window, invalidateShadow];
                let _: () = msg_send![window, setOpaque: objc::runtime::NO];
                // canJoinAllSpaces | stationary | ignoresCycle | fullScreenAuxiliary
                let behavior: usize = (1 << 0) | (1 << 4) | (1 << 6) | (1 << 8);
                let _: () = msg_send![window, setCollectionBehavior: behavior];
                return;
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (width, height, corner, margin);
    }
}

#[cfg(test)]
mod tests {
    use super::place_in_area;

    #[test]
    fn corners_stay_inside_the_work_area() {
        // A 1920x1080 monitor whose taskbar takes the bottom 48 px.
        let (l, t, r, b) = (0, 0, 1920, 1032);
        let (w, h, m) = (420, 58, 24);
        assert_eq!((place_in_area(l, r, w, m, true), place_in_area(t, b, h, m, true)), (24, 24));
        assert_eq!(
            (place_in_area(l, r, w, m, false), place_in_area(t, b, h, m, false)),
            (1920 - 420 - 24, 1032 - 58 - 24)
        );
    }

    #[test]
    fn work_area_offsets_are_respected() {
        // A monitor to the left of the primary one: negative coordinates.
        assert_eq!(place_in_area(-1920, 0, 420, 24, true), -1896);
        assert_eq!(place_in_area(-1920, 0, 420, 24, false), -444);
    }
}
