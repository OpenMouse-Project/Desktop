import type { ConnectedDevice } from "../native-hid/scan";

interface Props {
  connected: ConnectedDevice | null;
}

export function AppHeader(_props: Props) {
  return <header class="app-header-bar" />;
}
