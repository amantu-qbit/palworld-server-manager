// Regenerates src/data/palNames.json — the localized-display-name → Pal-icon-key
// index used to render live Pals on the World Map.
//
// Why this exists: the Palworld /v1/api/game-data endpoint reports each Pal's
// species in `Actor.Class` as its *localized display name* ("Petallia",
// "Blazehowl"), whereas the bundled icons in public/mapicons/pals are named by
// the game's internal code names ("flowerdoll", "manticore"). This script builds
// the bridge between them.
//
// Source of truth: oMaN-Rod/palworld-save-pal → data/json/l10n/en/pals.json
// (code_name → { localized_name }). Only entries whose cleansed code name has a
// matching bundled icon are included; on display-name collisions the base
// variant (fewest underscores, then shortest) wins.
//
// Usage:
//   node scripts/gen-pal-names.mjs path/to/en_pals.json
// (download en/pals.json from the palworld-save-pal repo first).

import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");

const srcPath = process.argv[2];
if (!srcPath) {
  console.error("usage: node scripts/gen-pal-names.mjs <en_pals.json>");
  process.exit(1);
}

// Mirrors cleanseCharacterId() in src/lib/mapData.ts.
const cleanse = (id) =>
  id
    .toLowerCase()
    .replace("predator_", "")
    .replace("_oilrig", "")
    .replace("raid_", "")
    .replace("summon_", "")
    .replace("_max", "")
    .replace(/_\d+$/, "")
    .replace("boss_", "")
    .replace("quest_farmer03_", "")
    .replace("_otomo", "");
const norm = (s) => s.toLowerCase().replace(/[^a-z0-9]/g, "");
const underscores = (s) => (s.match(/_/g) || []).length;

const pals = JSON.parse(readFileSync(srcPath, "utf8"));
const icons = new Set(
  readdirSync(resolve(root, "public/mapicons/pals"))
    .filter((f) => f.endsWith(".webp"))
    .map((f) => f.slice(0, -5)),
);

const candidates = {};
for (const [code, info] of Object.entries(pals)) {
  const name = info?.localized_name;
  if (!name) continue;
  const key = cleanse(code);
  if (!icons.has(key)) continue;
  (candidates[norm(name)] ??= []).push({ code, key });
}

const index = {};
for (const [nk, list] of Object.entries(candidates)) {
  list.sort(
    (a, b) =>
      underscores(a.code) - underscores(b.code) ||
      a.code.length - b.code.length ||
      a.code.localeCompare(b.code),
  );
  index[nk] = list[0].key;
}

const ordered = Object.fromEntries(Object.keys(index).sort().map((k) => [k, index[k]]));
writeFileSync(resolve(root, "src/data/palNames.json"), JSON.stringify(ordered));
console.log(`wrote src/data/palNames.json — ${Object.keys(ordered).length} entries`);
