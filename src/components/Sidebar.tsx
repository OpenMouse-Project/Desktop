import logo from "../assets/logo.png";

export function Sidebar() {
  return (
    <aside class="sidebar">
      <div class="sidebar-brand">
        <img class="brand-mark" src={logo} alt="" width={14} height={20} />
        <span class="brand-name">OpenMouse</span>
      </div>

      <div class="sidebar-empty">
        <p class="sidebar-empty-title">No devices connected</p>
        <p class="sidebar-empty-hint">
          Plug in a supported mouse to see it here.
        </p>
      </div>
    </aside>
  );
}
