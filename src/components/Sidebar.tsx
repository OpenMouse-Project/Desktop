export function Sidebar() {
  return (
    <aside class="sidebar">
      <div class="sidebar-brand">
        <span class="brand-mark" aria-hidden="true">
          Ʃ
        </span>
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
