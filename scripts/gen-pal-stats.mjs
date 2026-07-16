// Regenerates bridge/data/pal_stats.json — the slim per-species stat catalog
// the bridge needs to compute a pal's max HP (the heal endpoint) and max
// stomach, ported from palworld-save-pal's pal.py `max_hp` formula inputs.
//
//   { "<code>": { "hp": <scaling.hp>, "stomach": <max_full_stomach> }, ... }
//
// Source of truth: palworld-save-pal → data/json/pals.json.
//
// Usage: node scripts/gen-pal-stats.mjs <path-to-palworld-save-pal-checkout>

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const psp = process.argv[2];
if (!psp) {
  console.error("usage: node scripts/gen-pal-stats.mjs <palworld-save-pal checkout>");
  process.exit(1);
}

const pals = JSON.parse(readFileSync(join(psp, "data/json/pals.json"), "utf8"));

const out = {};
for (const [code, p] of Object.entries(pals)) {
  const hp = p?.scaling?.hp;
  const stomach = p?.max_full_stomach;
  if (typeof hp !== "number" && typeof stomach !== "number") continue;
  out[code] = {
    ...(typeof hp === "number" ? { hp } : {}),
    ...(typeof stomach === "number" ? { stomach } : {}),
  };
}

const dest = join(root, "bridge/data/pal_stats.json");
writeFileSync(dest, JSON.stringify(out, null, 1) + "\n");
console.log(`wrote ${dest}: ${Object.keys(out).length} species`);
