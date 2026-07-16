// Regenerates src/data/expTable.json — Palworld's cumulative EXP curve:
//   { cap, maxLevel, total[], palTotal[] }
// total[i] is the TotalEXP required to *be* player level i+1 (total[0] = 0 for
// level 1); palTotal[] is the same for Pals. cap is the displayed level cap
// (80 as of Palworld 1.0); levels above it exist in the table but are not
// reachable in normal play.
//
// Why: the Characters screen shows level progress bars; src/lib/expTable.ts
// turns a raw EXP value + level into "EXP into this level / EXP to next".
//
// Source of truth: a palworld-save-pal checkout — data/json/exp.json
// (dict keyed "1".."100" with TotalEXP / PalTotalEXP per level).
//
// Usage: node scripts/gen-exp-table.mjs <palworld-save-pal checkout>

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const src = process.argv[2];
if (!src) {
  console.error("usage: node scripts/gen-exp-table.mjs <palworld-save-pal checkout>");
  process.exit(1);
}

const CAP = 80;
const exp = JSON.parse(readFileSync(resolve(src, "data/json/exp.json"), "utf8"));
const maxLevel = Math.max(...Object.keys(exp).map(Number));

const total = [];
const palTotal = [];
for (let lv = 1; lv <= maxLevel; lv++) {
  const row = exp[String(lv)];
  if (!row) throw new Error(`exp.json missing level ${lv}`);
  total.push(row.TotalEXP);
  palTotal.push(row.PalTotalEXP);
}

writeFileSync(
  resolve(root, "src/data/expTable.json"),
  JSON.stringify({ cap: CAP, maxLevel, total, palTotal }),
);
console.log(`wrote src/data/expTable.json — cap ${CAP}, maxLevel ${maxLevel}, ${total.length} levels`);
