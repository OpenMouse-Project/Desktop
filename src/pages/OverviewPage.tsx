import { Mouse } from "lucide-preact";

export function OverviewPage() {
  return (
    <section class="page">
      <div class="empty-state">
        <Mouse class="empty-state-icon" size={40} aria-hidden="true" />
        <h2>No device connected</h2>
        <p>
          Connect a supported mouse over USB or a receiver to see its DPI,
          polling rate, battery, and firmware here.
        </p>
      </div>
    </section>
  );
}
