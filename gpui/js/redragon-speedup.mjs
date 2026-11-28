// @openmouse/protocol's Redragon button codec rebuilds its shortcut labels and
// re-encodes every option on each lookup, and some drivers do thousands of
// lookups while the bundle loads. Node hides it; QuickJS took 55 seconds, so no
// device appeared until then. These edits cache both, with identical results.
//
// Delete this once the fix is in a protocol release. If the file changes so the
// edits no longer apply, it is bundled as published and the build says so;
// tests/bundle.rs fails if that makes startup slow again.
import fs from "node:fs";

const EDITS = [
  [
    "    const options = [\n",
    "    const shortcutByName = new Map(SHORTCUTS.map((entry) => [shortcutName(entry), entry]));\n    const options = [\n",
  ],
  ["        ...SHORTCUTS.map(shortcutName),\n", "        ...shortcutByName.keys(),\n"],
  [
    "        const shortcut = SHORTCUTS.find((entry) => shortcutName(entry) === action);\n",
    "        const shortcut = shortcutByName.get(action);\n",
  ],
  ["    const decode = (value) => {\n", "    let encoded;\n    const decode = (value) => {\n"],
  [
    "        const named = options.find((option) => same(encode(option)));\n",
    "        encoded ??= options.map(encode);\n        const named = options[encoded.findIndex(same)];\n",
  ],
];

/** The faster source, or null when any edit does not match exactly once. */
export function speedUp(source) {
  let out = source.replace(/\r\n/g, "\n");
  for (const [from, to] of EDITS) {
    const parts = out.split(from);
    if (parts.length !== 2) return null;
    out = parts.join(to);
  }
  return out;
}

export const redragonSpeedup = {
  name: "redragon-speedup",
  setup(build) {
    build.onLoad({ filter: /protocol[\\/]dist[\\/]redragon[\\/]holtek\.js$/ }, (args) => {
      const faster = speedUp(fs.readFileSync(args.path, "utf8"));
      if (faster) return { contents: faster, loader: "js" };
      return { warnings: [{ text: "redragon-speedup no longer applies; see js/redragon-speedup.mjs" }] };
    });
  },
};
