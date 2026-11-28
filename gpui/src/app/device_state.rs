//! State the app keeps about connected devices.

/// Overview device state, driven by `crate::hid` (the shared protocol
/// drivers; see that module for how).
#[derive(Default)]
pub(super) struct Devices {
    pub(super) scanning: bool,
    /// Something changed during a scan, so look again when it finishes.
    pub(super) rescan: bool,
    pub(super) error: Option<String>,
    pub(super) tiles: Vec<Tile>,
    pub(super) selected: usize,
    /// Key of the device whose settings page is open, if any.
    pub(super) open: Option<String>,
    /// Edits made on that page and not yet written (instant apply off).
    pub(super) staged: Staged,
    /// X and Y DPI edited separately (mice with separate axes); linked by default,
    /// since a single number on screen hides a differing Y.
    pub(super) unlinked_axes: bool,
    pub(super) tab: DeviceTab,
    /// Stored profiles of the open Logitech device, once read (on demand).
    pub(super) onboard: Option<crate::hid::OnboardProfiles>,
    pub(super) onboard_loading: bool,
    /// The long-list control whose options are currently unfolded.
    pub(super) expanded: Option<String>,
    /// Label of the write in flight (disables further edits meanwhile).
    pub(super) writing: Option<String>,
    pub(super) write_error: Option<String>,
}

/// Pending, unwritten lighting edits (each `None` field is untouched).
#[derive(Default, Clone, PartialEq)]
pub(super) struct LightEdit {
    pub(super) mode: Option<String>,
    pub(super) color: Option<String>,
    pub(super) color2: Option<String>,
    pub(super) speed: Option<f64>,
}

/// Pending, unwritten edits on a device page.
#[derive(Default, Clone, PartialEq)]
pub(super) struct Staged {
    pub(super) dpi: Option<f64>,
    /// Y-axis DPI, only while the axes are unlinked.
    pub(super) dpi_y: Option<f64>,
    pub(super) polling: Option<u32>,
    pub(super) lod: Option<String>,
    pub(super) surface: Option<String>,
    pub(super) light: LightEdit,
}

impl Staged {
    pub(super) fn is_empty(&self) -> bool {
        *self == Staged::default()
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeviceTab {
    #[default]
    Performance,
    Buttons,
    Lighting,
    Advanced,
    /// The mouse's own stored profiles (Logitech).
    Profiles,
}

impl DeviceTab {
    pub(super) fn label(self) -> &'static str {
        match self {
            DeviceTab::Performance => "Performance",
            DeviceTab::Buttons => "Buttons",
            DeviceTab::Lighting => "Lighting",
            DeviceTab::Advanced => "Advanced",
            DeviceTab::Profiles => "Profiles",
        }
    }
}

pub(super) struct Tile {
    pub(super) key: String,
    pub(super) name: String,
    /// The registry's label for the driver; what gating and write routing use.
    pub(super) brand: String,
    /// The brand to show (the product's own — see `ConnectedDevice::display_brand`).
    pub(super) display_brand: String,
    pub(super) art: Option<LoadedArt>,
    pub(super) status: TileStatus,
}

pub(super) struct LoadedArt {
    pub(super) path: std::path::PathBuf,
    pub(super) placement: Option<crate::hid::ArtPlacement>,
}

pub(super) enum TileStatus {
    Idle,
    Connecting,
    Connected(DeviceReadout),
    Failed(String),
}

pub(super) struct DeviceReadout {
    /// Y-axis DPI, when the mouse reports one (it can differ from `dpi`).
    pub(super) dpi_y: Option<f64>,
    /// The mouse can hold X and Y sensitivity apart.
    pub(super) separate_axes: bool,
    /// The driver's own UI hints (`MouseUiHints`): what is meaningful for this
    /// protocol generation.
    pub(super) polling_read_only: bool,
    pub(super) hide_lod_low: bool,
    pub(super) lod_requires_surface: bool,
    /// A Logitech mouse with stored profiles the library can read.
    pub(super) onboard_profiles: bool,
    /// `Onboard`, `Host` or `Unknown` (Logitech).
    pub(super) mode: Option<String>,
    pub(super) tabs: crate::hid::Tabs,
    pub(super) controls: Option<crate::hid::Controls>,
    pub(super) polling_rates: Vec<u32>,
    pub(super) lod: Option<String>,
    pub(super) lod_options: Vec<String>,
    pub(super) surface: Option<String>,
    pub(super) battery: Option<f64>,
    pub(super) charging: bool,
    pub(super) dpi: Option<f64>,
    pub(super) polling_hz: Option<f64>,
    pub(super) connection: Option<String>,
    pub(super) firmware: Vec<String>,
}

impl DeviceReadout {
    pub(super) fn from_status(s: &serde_json::Value, controls: Option<crate::hid::Controls>, tabs: crate::hid::Tabs) -> Self {
        let num = |k: &str| s.get(k).and_then(|v| v.as_f64());
        let strings = |k: &str| -> Vec<String> {
            s.get(k)
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default()
        };
        let ui_flag = |k: &str| s.get("ui").and_then(|u| u.get(k)).and_then(|v| v.as_bool()).unwrap_or(false);
        Self {
            dpi_y: num("dpiY"),
            separate_axes: s.get("supportsSeparateDpiAxes").and_then(|v| v.as_bool()).unwrap_or(false),
            polling_read_only: ui_flag("pollingReadOnly"),
            hide_lod_low: ui_flag("hideLodLow"),
            lod_requires_surface: ui_flag("lodRequiresSurface"),
            onboard_profiles: s
                .get("onboardProfileFormat")
                .and_then(|f| f.get("supported"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                && s.get("brand").and_then(|b| b.as_str()) == Some("Logitech"),
            mode: s.get("deviceMode").and_then(|v| v.as_str()).map(str::to_string),
            tabs,
            controls,
            polling_rates: s
                .get("supportedPollingRates")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u32)).collect())
                .unwrap_or_default(),
            lod: s.get("liftOffDistance").and_then(|v| v.as_str()).map(str::to_string),
            lod_options: strings("supportedLiftOffDistances"),
            surface: s.get("gamingSurfaceMode").and_then(|v| v.as_str()).map(str::to_string),
            battery: num("batteryPercent"),
            charging: s
                .get("batteryState")
                .and_then(|v| v.as_str())
                .is_some_and(|b| b.starts_with("Charging")),
            dpi: num("dpi"),
            polling_hz: num("pollingRateHz"),
            connection: s.get("connectionType").and_then(|v| v.as_str()).map(str::to_string),
            firmware: s
                .get("firmware")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
        }
    }
}
