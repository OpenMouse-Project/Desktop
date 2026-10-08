import assert from "node:assert/strict";
import test from "node:test";
import { unsupportedKeyboardName } from "../src/native-hid/unsupported-keyboards.ts";

test("recognizes the Huntsman V3 Pro Mini even when its keyboard exposes a mouse interface", () => {
  assert.equal(unsupportedKeyboardName({ vendorId: 0x1532, productId: 0x02b0 }), "Razer Huntsman V3 Pro Mini");
});

test("keeps real Razer mice and other vendors eligible for their drivers", () => {
  assert.equal(unsupportedKeyboardName({ vendorId: 0x1532, productId: 0x00b2 }), null);
  assert.equal(unsupportedKeyboardName({ vendorId: 0x1532, productId: 0x00c1 }), null);
  assert.equal(unsupportedKeyboardName({ vendorId: 0x31e3, productId: 0x02b0 }), null);
});

test("other Huntsman keyboards use their reported model name", () => {
  assert.equal(unsupportedKeyboardName({
    vendorId: 0x1532,
    productId: 0x02a6,
    productString: "Razer Huntsman V3 Pro",
  }), "Razer Huntsman V3 Pro");
});
