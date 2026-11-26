// Fake mice for looking at the UI without hardware (OPENMOUSE_MOCK=slx, ulx,
// x3, mag, razer or egg; a trailing "!" hides the real devices). Nothing here
// talks to a device: writes are accepted and only the in-memory status changes.

export const isMockKey = (key: string) => key.startsWith("mock:");

/** Which fake devices OPENMOUSE_MOCK asked for ("slx", "ulx", "x3", "mag", "razer", "egg", or a comma list). */
export const mockCandidates = (wanted: string) => [
  ...(wanted.includes("slx") ? [{
    info: { key: "mock:slx", vendorId: 0x361d, productId: 0x0100, productString: "Starlight X (mock)", manufacturerString: "Finalmouse", collections: [] },
    brands: ["Finalmouse"],
  }] : []),
  ...(wanted.includes("ulx") ? [{
    info: { key: "mock:ulx", vendorId: 0x361d, productId: 0x0100, productString: "Finalmouse UltralightX (mock)", manufacturerString: "Finalmouse", collections: [] },
    brands: ["Finalmouse"],
  }] : []),
  ...(wanted.includes("mag") ? [{
    info: { key: "mock:mag", vendorId: 0x373b, productId: 0x1000, productString: "Magnetic mouse (mock)", manufacturerString: "Lamzu", collections: [] },
    brands: ["Lamzu"],
  }] : []),
  ...(wanted.includes("x3") ? [{
    info: { key: "mock:x3", vendorId: 0x046d, productId: 0xc0a9, productString: "PRO X 3 Superstrike (mock)", manufacturerString: "Logitech", collections: [] },
    brands: ["Logitech"],
  }] : []),
  ...(wanted.includes("razer") ? [{
    info: { key: "mock:razer", vendorId: 0x1532, productId: 0x00b8, productString: "Viper V3 Pro (mock)", manufacturerString: "Razer", collections: [] },
    brands: ["Razer"],
  }] : []),
  ...(wanted.includes("egg") ? [{
    info: { key: "mock:egg", vendorId: 0x3367, productId: 0x1941, productString: "OP1 8K v2 (mock)", manufacturerString: "Endgame Gear", collections: [] },
    brands: ["Endgame Gear"],
  }] : []),
];

export const mockBrand = (key: string) =>
  key === "mock:x3" ? "Logitech"
  : key === "mock:mag" ? "Lamzu"
  : key === "mock:razer" ? "Razer"
  : key === "mock:egg" ? "Endgame Gear"
  : "Finalmouse";

export const mockStatus = (key: string) =>
  key === "mock:x3" ? mockX3()
  : key === "mock:mag" ? mockMag()
  : key === "mock:ulx" ? mockUlx()
  : key === "mock:razer" ? mockRazer()
  : key === "mock:egg" ? mockEgg()
  : mockSlx();

// Ultralight X as FinalmouseHidClient.readStatus() reports it: the whole
// polling ladder and Motion Sync are adjustable, lift-off is the legacy 1/2 mm
// pair, and none of the SLX-only click/TMR/PAW/profile fields are present.
const mockUlx = () => ({
  brand: "Finalmouse",
  name: "Finalmouse UltralightX",
  ui: {
    family: "finalmouse-ulx",
    defaultDisplayName: "Finalmouse UltralightX",
    hideUnsupportedPollingRates: true,
    forceShowBattery: true,
    dpiStageEditor: { maxStages: 4, countEditable: true, minDpi: 10, maxDpi: 26000, stepDpi: 10 },
  },
  dpiStages: [400, 800, 1600, 3200],
  activeDpiStage: 1,
  batteryPercent: 91,
  batteryVoltageMv: 4080,
  batteryState: "Discharging",
  dpi: 1600,
  pollingRateHz: 2000,
  supportedPollingRates: [500, 1000, 2000, 4000, 8000],
  activeProfile: null,
  unitId: "ULX-0A1B2C3D",
  connectionType: "Wireless",
  connectionDetail: "2.4 GHz receiver · -52 dBm",
  motionSync: true,
  signalStrength: 3,
  liftOffDistance: "Medium",
  supportedLiftOffDistances: ["Medium", "High"],
  liftOffScale: null,
  finalmouseIsSlx: false,
  finalmouseDongleLedMode: 1,
  finalmouseTournamentScrollMode: 0,
  finalmouseTournamentScrollTimeoutMs: 500,
  firmware: ["Mouse 1.0.8", "Dongle RF 1.0.3", "Dongle USB 1.0.5"],
});

// A Hall-effect mouse as the Lamzu driver reports it: per-button trigger and
// release travel, rapid trigger in ms with no switch of its own, selectable
// switch type, and a sensor angle.
const mockMag = () => ({
  brand: "Lamzu",
  name: "Magnetic mouse",
  batteryPercent: 77,
  batteryState: "Discharging",
  connectionType: "Wireless",
  dpi: 1600,
  pollingRateHz: 1000,
  supportedPollingRates: [500, 1000, 2000, 4000],
  activeProfile: 1,
  angleTuning: 5,
  mockAngleTuningWritable: true,
  magneticButtons: {
    buttons: [
      { switchType: "magnetic", triggerPoint: 3, releasePoint: 3, rapidTrigger: 4, rapidTriggerEnabled: true },
      { switchType: "magnetic", triggerPoint: 5, releasePoint: null, rapidTrigger: null, rapidTriggerEnabled: false },
    ],
    triggerPointRange: { min: 1, max: 10 },
    releasePointRange: { min: 1, max: 10 },
    rapidTriggerRange: { min: 1, max: 15 },
    rapidTriggerUnit: "ms",
    rapidTriggerSwitch: false,
    canChooseSwitchType: true,
    calibration: "calibrated",
    liveDepth: false,
  },
});

// Razer's driver reports the two button families in one dict: cross-assignable
// controls and the toggle actions, which only offer their factory action or
// Disabled. Used by the mock to exercise both sections.
const mockRazer = () => ({
  brand: "Razer",
  name: "Viper V3 Pro",
  batteryPercent: 88,
  batteryState: "Discharging",
  connectionType: "Wireless",
  dpi: 1600,
  pollingRateHz: 1000,
  supportedPollingRates: [125, 250, 500, 1000, 2000, 4000, 8000],
  razerButtonMappings: {
    leftClick: "Left Click",
    rightClick: "Right Click",
    middleClick: "Middle Click",
    back: "Back",
    forward: "Forward",
    dpiCycle: "DPI Cycle",
    scrollUp: "Scroll Up",
    scrollDown: "Disabled",
    sensitivityButton: "Cycle Up Sensitivity Stages",
  },
  firmware: ["v2.01.00"],
});

// Endgame Gear v2 firmware: shared Glass/SPDT/CPI controls plus the sensor and
// handedness fields only the 8K v2 / 4K v2 wireless mice carry.
const mockEgg = () => ({
  brand: "Endgame Gear",
  name: "OP1 8K v2",
  batteryPercent: 76,
  batteryState: "Discharging",
  connectionType: "Wireless",
  dpi: 800,
  pollingRateHz: 8000,
  supportedPollingRates: [125, 250, 500, 1000, 2000, 4000, 8000],
  eggButtonMappings: ["Left Click", "Right Click", "Middle Click", "Back", "Forward", "DPI Cycle"],
  eggMulticlickFilters: [16, 16, 16, 16, 16],
  eggCpiStages: [{ dpi: 400 }, { dpi: 800 }, { dpi: 1600 }],
  eggCpiLevels: 3,
  eggPollingDivider: 1,
  eggGlassMode: false,
  eggSupportsGlassMode: true,
  slamclickFilter: false,
  motionJitterFilter: true,
  leftSpdtMode: "Off",
  rightSpdtMode: "GX Safe",
  eggSupportsV2SensorControls: true,
  eggAngleTuning: 0,
  eggForceMaxFps: false,
  eggLedLiftOffDisabled: false,
  eggLeftHanded: false,
  eggLodIndex: 3,
  eggLodOptions: ["0.7 mm", "0.8 mm", "1.0 mm", "1.5 mm", "2.0 mm"],
  firmware: ["v2.0.0"],
});

const mockX3 = () => ({
  brand: "Logitech",
  name: "PRO X 3 Superstrike",
  batteryPercent: 64,
  batteryState: "Discharging",
  connectionType: "Wireless",
  dpi: 1600,
  pollingRateHz: 4000,
  supportedPollingRates: [125, 250, 500, 1000, 2000, 4000, 8000],
  firmware: ["MPM 4.1.18", "BL1 3.0.4"],
  unitId: "3EC2C515",
  supportsSeparateDpiAxes: false,
  liftOffDistance: "Low",
  supportedLiftOffDistances: ["Low", "Medium", "High"],
  gamingSurfaceMode: "Auto",
  deviceMode: "Onboard",
  onboardProfileFormat: { supported: true, formatId: 8 },
  activeProfile: 1,
  analogButtonTuning: {
    maxActuation: 10,
    maxRapidTrigger: 5,
    maxHaptics: 5,
    buttons: [
      { actuation: 5, rapidTrigger: 2, haptics: 3, rapidTriggerEnabled: false },
      { actuation: 5, rapidTrigger: 2, haptics: 3, rapidTriggerEnabled: true },
    ],
  },
});

const mockSlx = () => ({
  brand: "Finalmouse",
  name: "Starlight X",
  batteryPercent: 82,
  batteryState: "Discharging",
  connectionType: "Wireless",
  dpi: 800,
  pollingRateHz: 1000,
  // PerfectPolling: fixed rate, nothing selectable (as the real driver reports it).
  supportedPollingRates: [] as number[],
  ui: { family: "finalmouse-slx", hideUnsupportedPollingRates: true, forceShowBattery: true, pollingReadOnly: true, hideMotionSync: true },
  finalmouseIsSlx: true,
  finalmouseDongleLedMode: 1,
  finalmouseClickModeL: 1,
  finalmouseClickModeR: 0,
  finalmouseClickReleaseL: 0,
  finalmouseClickReleaseR: 2,
  finalmouseTmrThrL: 15,
  finalmouseTmrThrR: 20,
  finalmouseTmrHystL: 200,
  finalmouseTmrHystR: 180,
  finalmouseTmrMspL: 420,
  finalmouseTmrMspR: 435,
  finalmousePawLodMm: 1.1,
  finalmousePawLodCustom: true,
  finalmouseProfileCount: 4,
  finalmouseProfileActive: 1,
  finalmouseProfileEnabledMask: 0b0111,
  finalmouseProfileNames: { 0: "FPS", 1: "Desktop", 2: "Tarkov" },
});

/** Stored profiles of the fake Superstrike, so the Profiles and Buttons tabs have something to show. */
let mockCurrent = 1;
let mockMode = "Onboard";
const actions = ["Left Click", "Right Click", "Middle Click", "Back", "Forward", "DPI Cycle"];
export const mockOnboard = (allActions: string[]) => ({
  profiles: [1, 2, 3].map((sector) => ({
    sector,
    enabled: true,
    isCurrent: sector === mockCurrent,
    name: ["Default", "Valorant", "Desktop"][sector - 1],
    dpiStages: [{ dpi: 400 }, { dpi: 800 }, { dpi: 1600 }, { dpi: 3200 }],
    defaultDpiIndex: 2,
    reportRateWireless: [4000, 8000, 1000][sector - 1],
    reportRateWired: 8000,
    buttonAssignments: actions.map((action) => ({ layer: "primary", button: 0, action })),
  })),
  actions: allActions,
});
export const mockOnboardAction = (action: string, value: any) => {
  if (action === "useProfile") { mockCurrent = Number(value); mockMode = "Onboard"; } else mockMode = value;
  return value;
};
