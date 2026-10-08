/** A keyboard seen on a mouse vendor's VID, without a matching keyboard driver. */
export function unsupportedKeyboardName(device: { vendorId: number; productId: number; productString?: string }): string | null {
  // Huntsman V3 Pro Mini also exposes a mouse HID interface for its controls.
  // The 0x1532 vendor fallback must not probe it with Razer mouse commands.
  if (device.vendorId !== 0x1532) return null;
  if (device.productId === 0x02b0) return "Razer Huntsman V3 Pro Mini";
  if (/\bhuntsman\b/i.test(device.productString ?? "")) return device.productString!;
  return null;
}
