// One tile in the Devices grid. Separated from OverviewPage because the
// artwork sizing needs per-image state: every product shot frames its mouse
// differently, so the tile measures the artwork's alpha bounds once (cached in
// device-images.ts) and sizes the image so the *mouse* — not the canvas —
// comes out the same size in every tile.
//
// The whole tile is the target: a transparent button covers it, which is what
// replaced the separate Connect/View button. The corner light is the only
// status affordance — steady green connected, slow pulse connecting, red
// failed.
import { useEffect, useState } from "preact/hooks";
import { Battery, type Gauge } from "lucide-preact";
import {
  artBounds,
  deviceArtStyle,
  deviceImage,
  deviceImageFallback,
  type ArtBounds,
} from "../native-hid/device-images";
import type { CandidateInterface } from "../native-hid/scan";

export type DeviceTileState = "idle" | "connecting" | "connected" | "error";

interface Props {
  candidate: CandidateInterface;
  /** What the device calls itself, when anything knows — see device-store. */
  displayName: string;
  features: { icon: typeof Gauge; label: string }[] | undefined;
  state: DeviceTileState;
  /** Only ever set for the connected device; nothing else has been read. */
  battery: number | null;
  onSelect: () => void;
}

function stateLabel(state: DeviceTileState, name: string): string {
  switch (state) {
    case "connected":
      return `View ${name}`;
    case "connecting":
      return `Connecting to ${name}`;
    case "error":
      return `${name} failed to connect — try again`;
    default:
      return `Connect to ${name}`;
  }
}

export function DeviceTile({ candidate, displayName, features, state, battery, onSelect }: Props) {
  const source = deviceImage(candidate.info.key, displayName);
  const [bounds, setBounds] = useState<ArtBounds | null>(null);
  const [artFailed, setArtFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    // Resolves from cache after the first tile on this machine has seen the
    // artwork, so only a genuinely new asset costs a decode.
    void artBounds(source).then((measured) => {
      if (!cancelled) setBounds(measured);
    });
    return () => {
      cancelled = true;
    };
  }, [source]);

  // No bounds, or the artwork never loaded: fall back to plainly containing it
  // in the tile rather than sizing it from measurements we do not have. A
  // failed load also stops the measured style from being applied, so the
  // placeholder is not blown up to product-shot size.
  const artStyle = artFailed ? null : deviceArtStyle(bounds);

  return (
    <li class={`device-card device-card--${state}`}>
      <button
        type="button"
        class="device-card-hit"
        disabled={state === "connecting"}
        onClick={onSelect}
        aria-label={stateLabel(state, displayName)}
      />
      {state !== "idle" && (
        <span class={`device-card-light device-card-light--${state}`} aria-hidden="true" />
      )}
      <div class="device-card-head">
        <span class="device-card-name">{displayName || "Unknown device"}</span>
        <div class="device-card-status">
          {battery != null && (
            <span class="device-card-battery">
              <Battery size={12} aria-hidden="true" />
              {battery}%
            </span>
          )}
          {/* Capability as icons on the status line: the label is the tooltip,
              so the tile stays artwork rather than a spec sheet. */}
          {features?.map((feature) => (
            <span
              class="device-card-feature"
              title={feature.label}
              aria-label={feature.label}
              key={feature.label}
            >
              <feature.icon size={12} aria-hidden="true" />
            </span>
          ))}
        </div>
        <span class="device-card-meta">
          {candidate.brands.join(" / ")} · {candidate.info.vendorId.toString(16).padStart(4, "0")}:
          {candidate.info.productId.toString(16).padStart(4, "0")}
        </span>
      </div>
      <div class="device-card-art">
        <img
          class={`device-card-image ${artStyle ? "" : "device-card-image--contain"}`}
          style={artStyle ?? undefined}
          src={source}
          onError={(event) => {
            setArtFailed(true);
            deviceImageFallback(event);
          }}
          alt=""
        />
      </div>
    </li>
  );
}
