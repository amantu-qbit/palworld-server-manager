// Regenerates the bundled technology catalogs shipped with the app:
//   src/data/techDex.json           — { code: { name, level, cost, boss, kind, icon, desc? } }
//   src/data/techNames.json         — { code: { name, level, boss } }
//   src/data/techAtlas.json         — { cols, cell, keys[] } grid metadata
//   public/mapicons/tech-atlas.webp — every referenced tech icon in one sheet
//
// Why: the Characters screen renders the in-game-style tech tree from these
// catalogs (src/lib/techDex.ts); techNames is the lighter name-only lookup.
//
// Source of truth: a palworld-save-pal checkout —
//   data/json/technologies.json           (level_cap, cost, is_boss_technology,
//                                          unlock_* arrays, icon)
//   data/json/l10n/en/technologies.json   (localized_name, description)
//   data/json/l10n/en/{items,buildings}.json (name fallback if a tech has no
//                                             localized name of its own)
//   ui/src/lib/assets/img/*.webp          (icon images; resized to 64px cells)
// Field derivation: level = level_cap, cost = cost, boss = is_boss_technology,
// kind = "structure" if unlock_build_objects is non-empty, else "item" if
// unlock_item_recipes is non-empty, else "other"; desc only when the English
// description is non-null.
//
// Usage: npm i --no-save sharp && node scripts/gen-tech-data.mjs <palworld-save-pal checkout>

import sharp from "sharp";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const src = process.argv[2];
if (!src) {
  console.error("usage: node scripts/gen-tech-data.mjs <palworld-save-pal checkout>");
  process.exit(1);
}
const IMG = resolve(src, "ui/src/lib/assets/img");
const CELL = 64;
const COLS = 24;

const json = (p) => JSON.parse(readFileSync(resolve(src, "data/json", p), "utf8"));
const tech = json("technologies.json");
const en = json("l10n/en/technologies.json");
const enItems = json("l10n/en/items.json");
const enBuildings = json("l10n/en/buildings.json");

const dex = {};
const names = {};
for (const code of Object.keys(tech).sort()) {
  const t = tech[code];
  const loc = en[code];
  const name =
    loc?.localized_name ||
    enItems[t.unlock_item_recipes?.[0]]?.localized_name ||
    enBuildings[t.unlock_build_objects?.[0]]?.localized_name ||
    code;
  if (!loc?.localized_name) console.warn(`no localized name: ${code} (using "${name}")`);
  const kind = t.unlock_build_objects?.length
    ? "structure"
    : t.unlock_item_recipes?.length
      ? "item"
      : "other";
  const entry = {
    name,
    level: t.level_cap,
    cost: t.cost,
    boss: t.is_boss_technology,
    kind,
    icon: t.icon || "",
  };
  if (loc?.description) entry.desc = loc.description;
  dex[code] = entry;
  names[code] = { name, level: t.level_cap, boss: t.is_boss_technology };
}
writeFileSync(resolve(root, "src/data/techDex.json"), JSON.stringify(dex));
writeFileSync(resolve(root, "src/data/techNames.json"), JSON.stringify(names));

// Atlas: sorted unique icons referenced by the dex that have a webp upstream.
const unique = [...new Set(Object.values(dex).map((e) => e.icon).filter(Boolean))].sort();
const keys = unique.filter((k) => existsSync(resolve(IMG, `${k}.webp`)));
for (const k of unique) if (!keys.includes(k)) console.warn(`no webp for icon: ${k}`);
const ROWS = Math.ceil(keys.length / COLS);

const cells = await Promise.all(
  keys.map((k) => sharp(resolve(IMG, `${k}.webp`)).resize(CELL, CELL).png().toBuffer()),
);
const buf = await sharp({
  create: { width: COLS * CELL, height: ROWS * CELL, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 0 } },
})
  .composite(cells.map((input, i) => ({ input, left: (i % COLS) * CELL, top: Math.floor(i / COLS) * CELL })))
  .webp({ quality: 88, effort: 6 })
  .toBuffer();

writeFileSync(resolve(root, "public/mapicons/tech-atlas.webp"), buf);
writeFileSync(resolve(root, "src/data/techAtlas.json"), JSON.stringify({ cols: COLS, cell: CELL, keys }));
console.log(
  `techDex/techNames: ${Object.keys(dex).length} entries; atlas: ${COLS * CELL}x${ROWS * CELL}, ` +
    `${keys.length} icons, ${(buf.length / 1024).toFixed(0)} KB`,
);
