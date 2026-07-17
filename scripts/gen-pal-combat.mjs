// Regenerates the two data files behind the Pal computed-stats display:
//
//   src/data/palScaling.json    { lowercode: { hp, attack, defense } }
//   src/data/passiveEffects.json { passiveCode: { attack, defense, work_speed } }
//
// palScaling is the per-species stat scaling; passiveEffects is each passive
// skill's self-targeted stat bonus (as a fraction, e.g. 0.2 = +20%). Both feed
// src/lib/palStats.ts, a direct port of palworld-save-pal's
// `ui/src/lib/utils/stats.ts::getStats`.
//
// Source of truth: a palworld-save-pal checkout —
//   data/json/pals.json           (scaling.{hp,attack,defense})
//   data/json/passive_skills.json (effects[].{type,value,target})
//
// Usage: node scripts/gen-pal-combat.mjs <palworld-save-pal checkout>

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const src = process.argv[2];
if (!src) {
  console.error("usage: node scripts/gen-pal-combat.mjs <palworld-save-pal checkout>");
  process.exit(1);
}

// --- per-species scaling -----------------------------------------------------
const pals = JSON.parse(readFileSync(resolve(src, "data/json/pals.json"), "utf8"));
const scaling = {};
for (const [code, p] of Object.entries(pals)) {
  const s = p?.scaling;
  if (!s || typeof s.hp !== "number") continue;
  scaling[code.toLowerCase()] = {
    hp: s.hp,
    attack: typeof s.attack === "number" ? s.attack : 0,
    defense: typeof s.defense === "number" ? s.defense : 0,
  };
}

// --- passive self-buff stat effects -----------------------------------------
// Mirrors getStats' classifiers and their evaluation ORDER (defense first, so
// ElementResist_* is never miscounted as attack) and its target filter
// (only self / self-and-trainer effects apply to the pal's own stats).
const isDefense = (t) => t === "Defense" || t.startsWith("ElementResist_");
const isAttack = (t) => t === "ShotAttack" || t.startsWith("Element") || t.startsWith("ElementBoost_");
const isWorkSpeed = (t) => t === "CraftSpeed";
const SELF_TARGETS = new Set(["ToSelf", "ToSelfAndTrainer"]);

const passives = JSON.parse(readFileSync(resolve(src, "data/json/passive_skills.json"), "utf8"));
const effects = {};
for (const [code, p] of Object.entries(passives)) {
  let attack = 0;
  let defense = 0;
  let work_speed = 0;
  for (const e of p?.effects ?? []) {
    if (!SELF_TARGETS.has(e.target)) continue;
    const v = (e.value ?? 0) / 100;
    if (isDefense(e.type)) defense += v;
    else if (isAttack(e.type)) attack += v;
    else if (isWorkSpeed(e.type)) work_speed += v;
  }
  if (attack || defense || work_speed) {
    effects[code] = {
      ...(attack ? { attack } : {}),
      ...(defense ? { defense } : {}),
      ...(work_speed ? { work_speed } : {}),
    };
  }
}

writeFileSync(resolve(root, "src/data/palScaling.json"), JSON.stringify(scaling));
writeFileSync(resolve(root, "src/data/passiveEffects.json"), JSON.stringify(effects));
console.log(
  `wrote src/data/palScaling.json (${Object.keys(scaling).length} species) + ` +
    `src/data/passiveEffects.json (${Object.keys(effects).length} passives)`,
);
