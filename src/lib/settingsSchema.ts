import { humanLabel } from "./format";

/** Display order for grouped settings; every key resolves to one of these. */
export const GROUP_ORDER = [
  "Rates",
  "Pals",
  "Players",
  "World & Items",
  "PvP & Guild",
  "Server & Network",
  "Misc",
] as const;

export type Group = (typeof GROUP_ORDER)[number];

/** Known Palworld setting keys → their group. Anything absent falls to "Misc". */
const KEY_GROUP: Record<string, Group> = {
  // Rates
  DayTimeSpeedRate: "Rates",
  NightTimeSpeedRate: "Rates",
  ExpRate: "Rates",
  WorkSpeedRate: "Rates",
  CollectionDropRate: "Rates",
  CollectionObjectHpRate: "Rates",
  CollectionObjectRespawnSpeedRate: "Rates",
  EnemyDropItemRate: "Rates",
  BuildObjectDamageRate: "Rates",
  BuildObjectDeteriorationDamageRate: "Rates",

  // Pals
  PalCaptureRate: "Pals",
  PalSpawnNumRate: "Pals",
  PalDamageRateAttack: "Pals",
  PalDamageRateDefense: "Pals",
  PalStomachDecreaceRate: "Pals",
  PalStaminaDecreaceRate: "Pals",
  PalAutoHPRegeneRate: "Pals",
  PalAutoHpRegeneRateInSleep: "Pals",
  PalEggDefaultHatchingTime: "Pals",

  // Players
  PlayerDamageRateAttack: "Players",
  PlayerDamageRateDefense: "Players",
  PlayerStomachDecreaceRate: "Players",
  PlayerStaminaDecreaceRate: "Players",
  PlayerAutoHPRegeneRate: "Players",
  PlayerAutoHpRegeneRateInSleep: "Players",
  DeathPenalty: "Players",
  bEnableFastTravel: "Players",
  bIsStartLocationSelectByMap: "Players",
  bExistPlayerAfterLogout: "Players",
  bEnableNonLoginPenalty: "Players",

  // World & Items
  Difficulty: "World & Items",
  DropItemMaxNum: "World & Items",
  DropItemMaxNum_UNKO: "World & Items",
  DropItemAliveMaxHours: "World & Items",
  BaseCampMaxNum: "World & Items",
  BaseCampWorkerMaxNum: "World & Items",
  bActiveUNKO: "World & Items",
  bEnableInvaderEnemy: "World & Items",
  bIsUseBackupSaveData: "World & Items",

  // PvP & Guild
  bIsPvP: "PvP & Guild",
  bEnablePlayerToPlayerDamage: "PvP & Guild",
  bEnableFriendlyFire: "PvP & Guild",
  bCanPickupOtherGuildDeathPenaltyDrop: "PvP & Guild",
  bEnableDefenseOtherGuildPlayer: "PvP & Guild",
  GuildPlayerMaxNum: "PvP & Guild",
  bAutoResetGuildNoOnlinePlayers: "PvP & Guild",
  AutoResetGuildTimeNoOnlinePlayers: "PvP & Guild",
  bEnableAimAssistPad: "PvP & Guild",
  bEnableAimAssistKeyboard: "PvP & Guild",

  // Server & Network
  ServerName: "Server & Network",
  ServerDescription: "Server & Network",
  ServerPlayerMaxNum: "Server & Network",
  CoopPlayerMaxNum: "Server & Network",
  bIsMultiplay: "Server & Network",
  PublicPort: "Server & Network",
  PublicIP: "Server & Network",
  Region: "Server & Network",
  RCONEnabled: "Server & Network",
  RCONPort: "Server & Network",
  bUseAuth: "Server & Network",
  BanListURL: "Server & Network",
  RESTAPIEnabled: "Server & Network",
  RESTAPIPort: "Server & Network",
  bShowPlayerList: "Server & Network",
  AllowConnectPlatform: "Server & Network",
  LogFormatType: "Server & Network",
};

/** Resolve a setting key to its group, defaulting to "Misc". */
export function groupOf(key: string): Group {
  return KEY_GROUP[key] ?? "Misc";
}

/** Human-readable label for a setting key. */
export function labelFor(key: string): string {
  return humanLabel(key);
}
