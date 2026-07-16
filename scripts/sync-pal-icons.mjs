// Syncs assets/pal-icons/ (the 64px Pal menu icons kept in the repo, out of
// the shipped bundle) from a palworld-save-pal checkout.
//
// Upstream ships them flat in ui/src/lib/assets/img/ as
// "t_<key>_icon_normal.webp" (128px as of Palworld 1.0); locally they are
// stored as "<key>.webp" at 64px — e.g. t_alpaca_icon_normal.webp ->
// alpaca.webp. Every upstream icon is copied in (overwriting stale versions);
// existing local icons with no upstream counterpart are left alone and
// reported. Run scripts/gen-pal-atlas.mjs afterwards to rebuild the atlas.
//
// Usage: npm i --no-save sharp && node scripts/sync-pal-icons.mjs <palworld-save-pal checkout>

import sharp from "sharp";
import { readdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const src = process.argv[2];
if (!src) {
  console.error("usage: node scripts/sync-pal-icons.mjs <palworld-save-pal checkout>");
  process.exit(1);
}
const IMG = resolve(src, "ui/src/lib/assets/img");
const DEST = resolve(root, "assets/pal-icons");
const SIZE = 64;

const upstream = readdirSync(IMG)
  .map((f) => f.match(/^t_(.+)_icon_normal\.webp$/))
  .filter(Boolean)
  .map((m) => m[1])
  .sort();

const existing = new Set(
  readdirSync(DEST).filter((f) => f.endsWith(".webp")).map((f) => f.slice(0, -5)),
);

let added = 0;
for (const key of upstream) {
  const img = sharp(resolve(IMG, `t_${key}_icon_normal.webp`));
  const meta = await img.metadata();
  const out =
    meta.width > SIZE || meta.height > SIZE
      ? await img.resize(SIZE, SIZE).webp({ quality: 90, effort: 6 }).toBuffer()
      : await img.webp({ quality: 90, effort: 6 }).toBuffer();
  writeFileSync(resolve(DEST, `${key}.webp`), out);
  if (!existing.has(key)) added++;
}

const orphans = [...existing].filter((k) => !upstream.includes(k));
console.log(
  `synced ${upstream.length} icons into assets/pal-icons (${added} new, ` +
    `${upstream.length - added} refreshed); ${orphans.length} local-only kept` +
    (orphans.length ? `: ${orphans.join(", ")}` : ""),
);
