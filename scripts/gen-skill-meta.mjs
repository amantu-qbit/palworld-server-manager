// Regenerates src/data/skillMeta.json — passive-skill ranks + disabled content:
//   { passiveRank:      { code: rank, ... }   — every passive's rank (-3..9)
//     disabledItems:    [static_id, ...]      — items flagged disabled upstream
//     disabledPassives: [code, ...]           — passives flagged disabled
//     disabledPals:     [character_id, ...] } — pals flagged disabled
//
// Why: the save editors color passive-skill options by rank (gold/teal/red like
// palworld-save-pal) and hide disabled/unreleased content from the item and
// passive pickers (ports palworld-save-pal PR #297's disabled-content filter).
// Typed access lives in src/lib/skillMeta.ts.
//
// Source of truth: a palworld-save-pal checkout —
//   data/json/passive_skills.json  (rank, disabled)
//   data/json/items.json           (disabled)
//   data/json/pals.json            (disabled)
//
// Usage: node scripts/gen-skill-meta.mjs <palworld-save-pal checkout>

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const src = process.argv[2];
if (!src) {
  console.error("usage: node scripts/gen-skill-meta.mjs <palworld-save-pal checkout>");
  process.exit(1);
}

const json = (p) => JSON.parse(readFileSync(resolve(src, "data/json", p), "utf8"));
const passives = json("passive_skills.json");
const items = json("items.json");
const pals = json("pals.json");

const passiveRank = {};
const disabledPassives = [];
for (const code of Object.keys(passives).sort()) {
  const p = passives[code];
  passiveRank[code] = p.rank ?? 0;
  if (p.disabled) disabledPassives.push(code);
}
const disabledItems = Object.keys(items).filter((k) => items[k].disabled).sort();
const disabledPals = Object.keys(pals).filter((k) => pals[k].disabled).sort();

writeFileSync(
  resolve(root, "src/data/skillMeta.json"),
  JSON.stringify({ passiveRank, disabledItems, disabledPassives, disabledPals }),
);
console.log(
  `skillMeta: ${Object.keys(passiveRank).length} passive ranks; disabled — ` +
    `${disabledItems.length} items, ${disabledPassives.length} passives, ${disabledPals.length} pals`,
);
