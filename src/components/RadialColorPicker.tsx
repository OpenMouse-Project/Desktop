import { useCallback, useEffect, useRef, useState } from "preact/hooks";

interface Props {
  value: string;
  onChange: (color: string) => void;
  disabled?: boolean;
}

/** Hex ↔ HSL helpers (no dependencies). */
function hexToHsl(hex: string): [number, number, number] {
  const r = parseInt(hex.slice(1, 3), 16) / 255;
  const g = parseInt(hex.slice(3, 5), 16) / 255;
  const b = parseInt(hex.slice(5, 7), 16) / 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h = 0;
  if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) / 6;
  else if (max === g) h = ((b - r) / d + 2) / 6;
  else h = ((r - g) / d + 4) / 6;
  return [h * 360, s * 100, l * 100];
}

function hslToHex(h: number, s: number, l: number): string {
  const sN = s / 100, lN = l / 100;
  const c = (1 - Math.abs(2 * lN - 1)) * sN;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = lN - c / 2;
  let r = 0, g = 0, b = 0;
  if (h < 60) { r = c; g = x; }
  else if (h < 120) { r = x; g = c; }
  else if (h < 180) { g = c; b = x; }
  else if (h < 240) { g = x; b = c; }
  else if (h < 300) { r = x; b = c; }
  else { r = c; b = x; }
  const toHex = (v: number) => Math.round((v + m) * 255).toString(16).padStart(2, "0");
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

const WHEEL_SIZE = 180;
const WHEEL_CX = WHEEL_SIZE / 2;
const WHEEL_CY = WHEEL_SIZE / 2;
const WHEEL_R = WHEEL_SIZE / 2 - 2;

export function RadialColorPicker({ value, onChange, disabled }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [h, s, l] = hexToHsl(value);
  const [dragging, setDragging] = useState<"wheel" | "lightness" | null>(null);

  // Draw the hue/saturation wheel.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const size = WHEEL_SIZE * 2; // 2x for crisp retina
    canvas.width = size;
    canvas.height = size;
    ctx.clearRect(0, 0, size, size);
    const cx = size / 2, cy = size / 2, radius = (WHEEL_R) * 2;

    // Draw hue wheel with saturation falling off from center.
    for (let angle = 0; angle < 360; angle += 1) {
      const startRad = ((angle - 0.5) * Math.PI) / 180;
      const endRad = ((angle + 1.5) * Math.PI) / 180;
      ctx.beginPath();
      ctx.moveTo(cx, cy);
      ctx.arc(cx, cy, radius, startRad, endRad);
      ctx.closePath();
      const grad = ctx.createRadialGradient(cx, cy, 0, cx, cy, radius);
      grad.addColorStop(0, `hsl(${angle}, 0%, 100%)`);
      grad.addColorStop(0.5, `hsl(${angle}, 100%, 50%)`);
      grad.addColorStop(1, `hsl(${angle}, 100%, 50%)`);
      ctx.fillStyle = grad;
      ctx.fill();
    }

    // Cut out the center to make a ring.
    ctx.globalCompositeOperation = "destination-out";
    ctx.beginPath();
    ctx.arc(cx, cy, radius * 0.15, 0, Math.PI * 2);
    ctx.fill();
    ctx.globalCompositeOperation = "source-over";
  }, []);

  const pickFromWheel = useCallback((clientX: number, clientY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const x = ((clientX - rect.left) / rect.width) * WHEEL_SIZE - WHEEL_CX;
    const y = ((clientY - rect.top) / rect.height) * WHEEL_SIZE - WHEEL_CY;
    const dist = Math.sqrt(x * x + y * y);
    if (dist > WHEEL_R || dist < WHEEL_R * 0.15) return;
    const angle = (Math.atan2(y, x) * 180) / Math.PI;
    const hue = (angle + 360) % 360;
    const sat = Math.min(100, (dist / WHEEL_R) * 100);
    onChange(hslToHex(hue, sat, l));
  }, [l, onChange]);

  useEffect(() => {
    if (dragging !== "wheel") return;
    const onMove = (e: MouseEvent | TouchEvent) => {
      const pt = "touches" in e ? e.touches[0] : e;
      pickFromWheel(pt.clientX, pt.clientY);
    };
    const onUp = () => setDragging(null);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    window.addEventListener("touchmove", onMove);
    window.addEventListener("touchend", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      window.removeEventListener("touchmove", onMove);
      window.removeEventListener("touchend", onUp);
    };
  }, [dragging, pickFromWheel]);

  // Lightness slider drag.
  const sliderRef = useRef<HTMLDivElement>(null);
  const hRef = useRef(h);
  const sRef = useRef(s);
  hRef.current = h;
  sRef.current = s;

  const pickFromSlider = useCallback((clientX: number) => {
    const el = sliderRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const pct = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    onChange(hslToHex(hRef.current, sRef.current, pct * 100));
  }, [onChange]);

  useEffect(() => {
    if (dragging !== "lightness") return;
    const onMove = (e: MouseEvent | TouchEvent) => {
      const pt = "touches" in e ? e.touches[0] : e;
      pickFromSlider(pt.clientX);
    };
    const onUp = () => setDragging(null);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    window.addEventListener("touchmove", onMove);
    window.addEventListener("touchend", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      window.removeEventListener("touchmove", onMove);
      window.removeEventListener("touchend", onUp);
    };
  }, [dragging, pickFromSlider]);

  // Marker position on the wheel.
  const markerDist = (s / 100) * WHEEL_R;
  const markerRad = (h * Math.PI) / 180;
  const markerX = WHEEL_CX + Math.cos(markerRad) * markerDist;
  const markerY = WHEEL_CY + Math.sin(markerRad) * markerDist;

  return (
    <div class="radial-picker">
      <div class="radial-picker-wheel-wrap">
        <canvas
          ref={canvasRef}
          class="radial-picker-wheel"
          style={{ width: WHEEL_SIZE, height: WHEEL_SIZE }}
          onMouseDown={(e) => { if (!disabled) { setDragging("wheel"); pickFromWheel(e.clientX, e.clientY); } }}
          onTouchStart={(e) => { if (!disabled) { setDragging("wheel"); pickFromWheel(e.touches[0].clientX, e.touches[0].clientY); } }}
        />
        <div
          class="radial-picker-marker"
          style={{
            left: markerX,
            top: markerY,
            backgroundColor: value,
            borderColor: l > 60 ? "#000" : "#fff",
          }}
        />
      </div>
      <div class="radial-picker-lightness-wrap">
        <span class="radial-picker-lightness-label">Lightness</span>
        <div
          ref={sliderRef}
          class="radial-picker-lightness-track"
          onMouseDown={(e) => { if (!disabled) { setDragging("lightness"); pickFromSlider(e.clientX); } }}
          onTouchStart={(e) => { if (!disabled) { setDragging("lightness"); pickFromSlider(e.touches[0].clientX); } }}
        >
          <div class="radial-picker-lightness-gradient" style={{ background: `linear-gradient(to right, hsl(${h},${s}%,0%), hsl(${h},${s}%,50%), hsl(${h},${s}%,100%))` }} />
          <div
            class="radial-picker-lightness-thumb"
            style={{ left: `${l}%`, backgroundColor: value }}
          />
        </div>
      </div>
      <div class="radial-picker-preview">
        <div class="radial-picker-swatch" style={{ backgroundColor: value }} />
        <span class="radial-picker-hex">{value.toUpperCase()}</span>
      </div>
    </div>
  );
}
