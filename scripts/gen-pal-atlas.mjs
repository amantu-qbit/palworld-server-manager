// Regenerates the Pal-icon sprite atlas shipped with the app:
//   public/mapicons/pals-atlas.webp  — all icons packed into one image
//   src/data/palAtlas.json           — { cols, cell, keys[] } grid metadata
//   src/data/palIconKeys.json        — the same keys[] as a standalone list
//
// Every Pal icon lives in this single sheet so the map loads them in one request
// instead of one-per-species. Source icons (64px, from palworld-save-pal) live in
// assets/pal-icons/ (kept in the repo but out of the shipped bundle).
//
// Usage: npm i --no-save sharp && node scripts/gen-pal-atlas.mjs

import sharp from "sharp";
import { readdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = resolve(root, "assets/pal-icons");
const CELL = 64;
const COLS = 22;

const keys = readdirSync(SRC)
  .filter((f) => f.endsWith(".webp"))
  .map((f) => f.slice(0, -5))
  .sort();
const ROWS = Math.ceil(keys.length / COLS);

const buf = await sharp({
  create: { width: COLS * CELL, height: ROWS * CELL, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 0 } },
})
  .composite(keys.map((k, i) => ({ input: `${SRC}/${k}.webp`, left: (i % COLS) * CELL, top: Math.floor(i / COLS) * CELL })))
  .webp({ quality: 88, effort: 6 })
  .toBuffer();

writeFileSync(resolve(root, "public/mapicons/pals-atlas.webp"), buf);
writeFileSync(resolve(root, "src/data/palAtlas.json"), JSON.stringify({ cols: COLS, cell: CELL, keys }));
writeFileSync(resolve(root, "src/data/palIconKeys.json"), JSON.stringify(keys));
console.log(`atlas: ${COLS * CELL}x${ROWS * CELL}, ${keys.length} icons, ${(buf.length / 1024).toFixed(0)} KB`);
