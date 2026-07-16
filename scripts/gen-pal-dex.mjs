// Regenerates src/data/palDex.json — the bundled Pal catalog:
//   { lowercased_code_name: { name, elements, rarity } }
//
// Why: the bridge reports each Pal by its internal code name ("SheepBall",
// "BOSS_Alpaca"); src/lib/palDex.ts cleanses that id and looks it up here to
// get the display name ("Lamball"), element types, and rarity. Every entry in
// upstream pals.json is kept (including humans and quest/boss variants) so
// any character the bridge can report resolves to a name.
//
// Source of truth: a palworld-save-pal checkout —
//   data/json/pals.json          (element_types, rarity per code name)
//   data/json/l10n/en/pals.json  (localized_name)
//
// Usage: node scripts/gen-pal-dex.mjs <palworld-save-pal checkout>

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const src = process.argv[2];
if (!src) {
  console.error("usage: node scripts/gen-pal-dex.mjs <palworld-save-pal checkout>");
  process.exit(1);
}

const pals = JSON.parse(readFileSync(resolve(src, "data/json/pals.json"), "utf8"));
const en = JSON.parse(readFileSync(resolve(src, "data/json/l10n/en/pals.json"), "utf8"));

const dex = {};
const codes = Object.keys(pals).sort((a, b) => (a.toLowerCase() < b.toLowerCase() ? -1 : 1));
for (const code of codes) {
  const key = code.toLowerCase();
  const name = en[code]?.localized_name;
  if (!name) {
    console.warn(`no localized name, skipping: ${code}`);
    continue;
  }
  if (dex[key]) console.warn(`duplicate key after lowercasing: ${code} -> ${key}`);
  dex[key] = { name, elements: pals[code].element_types ?? [], rarity: pals[code].rarity ?? 0 };
}

writeFileSync(resolve(root, "src/data/palDex.json"), JSON.stringify(dex));
console.log(`wrote src/data/palDex.json — ${Object.keys(dex).length} entries`);
