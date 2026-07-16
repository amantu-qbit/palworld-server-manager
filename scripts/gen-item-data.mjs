// Regenerates the bundled item-icon catalogs shipped with the app:
//   src/data/itemIcons.json          — { item_static_id: icon_base_name }
//   src/data/itemAtlas.json          — { cols, cell, keys[] } grid metadata
//   public/mapicons/items-atlas.webp — every available item icon in one sheet
//
// Why: the bridge reports inventory items by internal static_id; the UI
// resolves each id to its icon cell via itemIcons.json + itemAtlas.json
// (src/lib/itemDex.ts) so all icons load in a single request.
//
// Source of truth: a palworld-save-pal checkout —
//   data/json/items.json         ("icon" field per static_id)
//   ui/src/lib/assets/img/*.webp (the icon images themselves)
// Every item keeps its itemIcons.json entry even when no webp exists (the UI
// falls back to a placeholder); only icons with an actual webp go in the atlas.
// Source images may be larger than 64px (256px as of Palworld 1.0) and are
// resized down to the 64px atlas cell.
//
// Usage: npm i --no-save sharp && node scripts/gen-item-data.mjs <palworld-save-pal checkout>

import sharp from "sharp";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const src = process.argv[2];
if (!src) {
  console.error("usage: node scripts/gen-item-data.mjs <palworld-save-pal checkout>");
  process.exit(1);
}
const IMG = resolve(src, "ui/src/lib/assets/img");
const CELL = 64;
const COLS = 30;

const items = JSON.parse(readFileSync(resolve(src, "data/json/items.json"), "utf8"));

// { static_id: icon_base_name }, sorted by static_id — all items, icon or not.
const icons = {};
for (const id of Object.keys(items).sort()) {
  const icon = items[id]?.icon;
  if (icon) icons[id] = icon;
  else console.warn(`no icon field: ${id}`);
}
writeFileSync(resolve(root, "src/data/itemIcons.json"), JSON.stringify(icons));

// Atlas: sorted unique icon names that actually have a webp upstream.
const unique = [...new Set(Object.values(icons))].sort();
const keys = unique.filter((k) => existsSync(resolve(IMG, `${k}.webp`)));
const missing = unique.length - keys.length;
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

writeFileSync(resolve(root, "public/mapicons/items-atlas.webp"), buf);
writeFileSync(resolve(root, "src/data/itemAtlas.json"), JSON.stringify({ cols: COLS, cell: CELL, keys }));
console.log(
  `itemIcons: ${Object.keys(icons).length} entries; atlas: ${COLS * CELL}x${ROWS * CELL}, ` +
    `${keys.length} icons (${missing} without webp, kept out of atlas), ${(buf.length / 1024).toFixed(0)} KB`,
);
