/**
 * Realistic demo data for the mock adapter (used in browser dev/preview
 * and anywhere the app runs without a live server).
 */
import type { Actor, GameData, Metrics, Player, ServerInfo, Settings } from "../types/api";

export const serverInfo: ServerInfo = {
  version: "v0.6.3",
  servername: "Amir's Palpagos Islands",
  description: "Casual co-op survival. Be kind, catch Pals, build big.",
  worldguid: "3F9AD21B7C4E48A9B0E15C2277C0FA21",
};

export const baseMetrics: Metrics = {
  serverfps: 59,
  currentplayernum: 12,
  serverframetime: 16.9,
  maxplayernum: 32,
  uptime: 4 * 86400 + 12 * 3600 + 37 * 60, // 4d 12h 37m
};

export const settings: Settings = {
  Difficulty: "None",
  DayTimeSpeedRate: 1,
  NightTimeSpeedRate: 1,
  ExpRate: 1.5,
  PalCaptureRate: 1.2,
  PalSpawnNumRate: 1,
  PalDamageRateAttack: 1,
  PalDamageRateDefense: 1,
  PlayerDamageRateAttack: 1,
  PlayerDamageRateDefense: 1,
  PlayerStomachDecreaceRate: 1,
  PlayerStaminaDecreaceRate: 1,
  PlayerAutoHPRegeneRate: 1,
  PlayerAutoHpRegeneRateInSleep: 1,
  PalStomachDecreaceRate: 1,
  PalStaminaDecreaceRate: 1,
  PalAutoHPRegeneRate: 1,
  PalAutoHpRegeneRateInSleep: 1,
  BuildObjectDamageRate: 1,
  BuildObjectDeteriorationDamageRate: 1,
  CollectionDropRate: 1.5,
  CollectionObjectHpRate: 1,
  CollectionObjectRespawnSpeedRate: 1,
  EnemyDropItemRate: 1,
  DeathPenalty: "Item",
  bEnablePlayerToPlayerDamage: false,
  bEnableFriendlyFire: false,
  bEnableInvaderEnemy: true,
  bActiveUNKO: false,
  bEnableAimAssistPad: true,
  bEnableAimAssistKeyboard: false,
  DropItemMaxNum: 3000,
  DropItemMaxNum_UNKO: 100,
  BaseCampMaxNum: 128,
  BaseCampWorkerMaxNum: 15,
  DropItemAliveMaxHours: 1,
  bAutoResetGuildNoOnlinePlayers: false,
  AutoResetGuildTimeNoOnlinePlayers: 72,
  GuildPlayerMaxNum: 20,
  PalEggDefaultHatchingTime: 48,
  WorkSpeedRate: 1.25,
  bIsMultiplay: false,
  bIsPvP: false,
  bCanPickupOtherGuildDeathPenaltyDrop: false,
  bEnableNonLoginPenalty: true,
  bEnableFastTravel: true,
  bIsStartLocationSelectByMap: true,
  bExistPlayerAfterLogout: false,
  bEnableDefenseOtherGuildPlayer: false,
  CoopPlayerMaxNum: 4,
  ServerPlayerMaxNum: 32,
  ServerName: "Amir's Palpagos Islands",
  ServerDescription: "Casual co-op survival. Be kind, catch Pals, build big.",
  PublicPort: 8211,
  PublicIP: "",
  RCONEnabled: true,
  RCONPort: 25575,
  Region: "",
  bUseAuth: true,
  BanListURL: "https://api.palworldgame.com/api/banlist.txt",
  RESTAPIEnabled: true,
  RESTAPIPort: 8212,
  bShowPlayerList: true,
  AllowConnectPlatform: "Steam",
  bIsUseBackupSaveData: true,
  LogFormatType: "Text",
};

const PLAYER_NAMES = [
  "Nyx", "Riven", "Sol", "Koda", "Amir", "Bea",
  "Juno", "Kade", "Mira", "Otto", "Pax", "Wren",
];
const GUILDS = ["Palpagos Pioneers", "Night Owls", "The Free Pals", "Kindling"];

// World-coordinate anchors that fall on the Palpagos landmasses, so demo actors
// sit on islands rather than in the ocean. (Real-server data is exact regardless.)
const LAND = [
  { x: -178968, y: 102920 }, // central green island (base)
  { x: -197328, y: 378320 }, // snowy island (north)
  { x: 78072, y: 341600 }, // desert island (north-east)
  { x: -380928, y: 11120 }, // volcano island (south-west)
  { x: 96432, y: 84560 }, // eastern isles
  { x: -178968, y: -99040 }, // southern isles
  { x: -32088, y: 176360 }, // mid-right green
] as const;
const BASE = LAND[0];

export const players: Player[] = PLAYER_NAMES.map((name, i) => ({
  name,
  accountName: `${name.toLowerCase()}_steam`,
  playerId: (0x1000 + i * 7).toString(16).toUpperCase().padStart(8, "0"),
  userId: `steam_7656119${(80000000000 + i * 137).toString().slice(0, 10)}`,
  ip: `192.168.1.${20 + i}`,
  ping: [12, 24, 33, 41, 58, 62, 77, 19, 28, 45, 51, 90][i],
  location_x: Math.round(LAND[i % LAND.length].x + Math.cos(i * 1.7) * 26000),
  location_y: Math.round(LAND[i % LAND.length].y + Math.sin(i * 1.7) * 26000),
  level: [48, 51, 12, 39, 50, 27, 44, 8, 33, 50, 22, 16][i],
  building_count: [140, 95, 3, 78, 210, 40, 120, 0, 65, 188, 22, 11][i],
}));

/* ---- Deterministic actor generation (seeded, so the map is stable) ---- */
function mulberry32(seed: number) {
  return function () {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const PAL_SPECIES = [
  "Lamball", "Cattiva", "Chikipi", "Foxparks", "Fuack", "Sparkit", "Tanzee", "Depresso",
  "Vixy", "Hoocrates", "Teafant", "Direhowl", "Pengullet", "Jolthog", "Gumoss", "Tocotoco",
  "Flambelle", "Melpaca", "Eikthyrdeer", "Nitewing", "Mau", "Robinquill", "Gorirat", "Rushoar",
  "Grintale", "Rooby", "Mozzarina", "Dumud", "Cawgnito", "Leezpunk", "Loupmoon", "Galeclaw",
  "Reptyro", "Kingpaca", "Mammorest", "Verdash", "Vanwyrm", "Bushi", "Beakon", "Ragnahawk",
  "Katress", "Wixen", "Lunaris",
];
const ACTIONS = ["Idle", "Wandering", "Working", "Combat", "Sleeping", "Eating", "Mining", "Watering"];

function makeActor(
  rnd: () => number,
  i: number,
  kind: Actor["UnitType"],
  anchor: { x: number; y: number },
  spread: number,
): Actor {
  const species = PAL_SPECIES[Math.floor(rnd() * PAL_SPECIES.length)];
  const level = 1 + Math.floor(rnd() * 50);
  const maxhp = level * 120 + 200;
  return {
    Type: "Character",
    InstanceID: `A${(i + 1).toString(16).toUpperCase().padStart(6, "0")}`,
    UnitType: kind,
    NickName: species,
    Class: species,
    level,
    HP: Math.floor(maxhp * (0.35 + rnd() * 0.65)),
    MaxHP: maxhp,
    GuildName: kind === "BaseCampPal" ? GUILDS[Math.floor(rnd() * GUILDS.length)] : undefined,
    Action: ACTIONS[Math.floor(rnd() * ACTIONS.length)],
    AI_Action: kind === "WildPal" ? "Roam" : "Assigned",
    LocationX: Math.round(anchor.x + (rnd() - 0.5) * 2 * spread),
    LocationY: Math.round(anchor.y + (rnd() - 0.5) * 2 * spread),
    LocationZ: Math.round((rnd() - 0.4) * 40000),
    IsActive: "true",
  };
}

export function generateActors(seed = 20260710): Actor[] {
  const rnd = mulberry32(seed);
  const out: Actor[] = [];

  // Players (mirror the roster positions)
  players.forEach((p, i) => {
    out.push({
      Type: "Character",
      InstanceID: `P${(i + 1).toString(16).toUpperCase().padStart(6, "0")}`,
      UnitType: "Player",
      NickName: p.name,
      userid: p.userId,
      ip: p.ip,
      level: p.level,
      HP: 500 + p.level * 10,
      MaxHP: 500 + p.level * 10,
      GuildName: GUILDS[i % GUILDS.length],
      Action: ACTIONS[Math.floor(rnd() * ACTIONS.length)],
      LocationX: p.location_x,
      LocationY: p.location_y,
      LocationZ: Math.round((rnd() - 0.4) * 20000),
      IsActive: "true",
    });
  });

  let idx = 0;
  const randAnchor = () => LAND[Math.floor(rnd() * LAND.length)];
  const add = (kind: Actor["UnitType"], n: number, pick: () => { anchor: { x: number; y: number }; spread: number }) => {
    for (let k = 0; k < n; k++) {
      const { anchor, spread } = pick();
      out.push(makeActor(rnd, idx++, kind, anchor, spread));
    }
  };
  add("WildPal", 120, () => ({ anchor: randAnchor(), spread: 62000 }));
  add("BaseCampPal", 34, () => ({ anchor: BASE, spread: 22000 }));
  add("OtomoPal", 10, () => ({ anchor: BASE, spread: 16000 }));
  add("NPC", 8, () => ({ anchor: randAnchor(), spread: 30000 }));
  return out;
}

export const gameData: GameData = {
  Time: "2026-07-10 11:12:44",
  FPS: 59,
  AverageFPS: 58.4,
  ActorData: generateActors(),
};
