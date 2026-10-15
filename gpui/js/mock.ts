// A fake Finalmouse Starlight X for looking at the UI without hardware
// (OPENMOUSE_MOCK=slx). Nothing here talks to a device: writes are accepted
// and only the in-memory status changes.

export const isMockKey = (key: string) => key.startsWith("mock:");

/** Which fake devices OPENMOUSE_MOCK asked for ("slx", "x3", or "slx,x3"). */
export const mockCandidates = (wanted: string) => [
  ...(wanted.includes("slx") ? [{
    info: { key: "mock:slx", vendorId: 0x361d, productId: 0x0100, productString: "Starlight X (mock)", manufacturerString: "Finalmouse", collections: [] },
    brands: ["Finalmouse"],
  }] : []),
  ...(wanted.includes("x3") ? [{
    info: { key: "mock:x3", vendorId: 0x046d, productId: 0xc0a9, productString: "PRO X 3 Superstrike (mock)", manufacturerString: "Logitech", collections: [] },
    brands: ["Logitech"],
  }] : []),
];

export const mockBrand = (key: string) => (key === "mock:x3" ? "Logitech" : "Finalmouse");

export const mockStatus = (key: string) => (key === "mock:x3" ? mockX3() : mockSlx());

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
