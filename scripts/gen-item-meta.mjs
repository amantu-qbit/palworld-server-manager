// Regenerates src/data/itemMeta.json — per-item weight + max stack size:
//   { static_id: { weight: <number>, stack: <max_stack_count> }, ... }
//
// Why: the in-game inventory shows a small weight number in the top-left of
// every item slot; the bridge reports inventory items only by internal
// static_id, so the UI needs a bundled catalog to render each item's per-unit
// weight (and stack ceiling) without a round-trip. Typed access lives in
// src/lib/itemMeta.ts.
//
// Source of truth: a palworld-save-pal checkout —
//   data/json/items.json  ("weight" + "max_stack_count" per static_id)
// Every item with a numeric weight is kept, sorted by static_id.
//
// Usage: node scripts/gen-item-meta.mjs <palworld-save-pal checkout>

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const src = process.argv[2];
if (!src) {
  console.error("usage: node scripts/gen-item-meta.mjs <palworld-save-pal checkout>");
  process.exit(1);
}

const items = JSON.parse(readFileSync(resolve(src, "data/json/items.json"), "utf8"));

// { static_id: { weight, stack } }, sorted by static_id — every item that
// carries a numeric weight (items without one are skipped and warned about).
const meta = {};
for (const id of Object.keys(items).sort()) {
  const weight = items[id]?.weight;
  if (typeof weight !== "number") {
    console.warn(`no numeric weight: ${id}`);
    continue;
  }
  meta[id] = { weight, stack: items[id]?.max_stack_count ?? 0 };
}

writeFileSync(resolve(root, "src/data/itemMeta.json"), JSON.stringify(meta, null, 2) + "\n");
console.log(`itemMeta: ${Object.keys(meta).length} entries`);
