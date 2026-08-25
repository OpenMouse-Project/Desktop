import logo from "../assets/logo.png";
import type { ConnectedDevice } from "../native-hid/scan";
import { deviceImage, UNKNOWN_DEVICE_IMAGE } from "../native-hid/device-images";

interface Props {
  connected: ConnectedDevice | null;
  /** Jump to this device's card — no-op target when nothing is connected. */
  onSelectDevice: () => void;
}

function fallbackToUnknownDevice(event: Event) {
  const img = event.currentTarget as HTMLImageElement;
  if (img.src.endsWith(UNKNOWN_DEVICE_IMAGE)) return;
  img.src = UNKNOWN_DEVICE_IMAGE;
}

export function Sidebar({ connected, onSelectDevice }: Props) {
  return (
    <aside class="sidebar">
      <div class="sidebar-brand">
        <img class="brand-mark" src={logo} alt="" width={14} height={20} />
        <span class="brand-name">OpenMouse</span>
      </div>

      {connected ? (
        <button class="sidebar-device" onClick={onSelectDevice} title="View this device">
          <span class="sidebar-device-icon">
            <img
              class="sidebar-device-photo"
              src={deviceImage(connected.key, connected.status.name)}
              onError={fallbackToUnknownDevice}
              alt=""
            />
            <span class="sidebar-device-dot" aria-hidden="true" />
          </span>
          <span class="sidebar-device-info">
            <span class="sidebar-device-name">{connected.status.name}</span>
            <span class="sidebar-device-brand">{connected.brand}</span>
          </span>
        </button>
      ) : (
        <div class="sidebar-empty">
          <p class="sidebar-empty-title">No devices connected</p>
          <p class="sidebar-empty-hint">
            Plug in a supported mouse to see it here.
          </p>
        </div>
      )}
    </aside>
  );
}
