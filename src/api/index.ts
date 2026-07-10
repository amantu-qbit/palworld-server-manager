import type {
  ActionResult,
  Connection,
  GameData,
  Metrics,
  Player,
  ServerInfo,
  Settings,
} from "../types/api";
import { mockApi } from "./mock";
import { tauriApi } from "./tauri";
import { httpApi } from "./http";

/**
 * The single surface every screen talks to. Three implementations exist:
 *  - `tauriApi` — real; Rust commands do the HTTP Basic-auth request (desktop app).
 *  - `httpApi`  — real; browser build talks to a real server via the Vite dev proxy.
 *  - `mockApi`  — realistic demo data, used only when the user opts into demo mode.
 */
export interface PalworldApi {
  testConnection(c: Connection): Promise<ActionResult>;
  getInfo(): Promise<ServerInfo>;
  getMetrics(): Promise<Metrics>;
  getPlayers(): Promise<Player[]>;
  getSettings(): Promise<Settings>;
  getGameData(): Promise<GameData>;
  announce(message: string): Promise<ActionResult>;
  kick(userid: string, message: string): Promise<ActionResult>;
  ban(userid: string, message: string): Promise<ActionResult>;
  unban(userid: string): Promise<ActionResult>;
  saveWorld(): Promise<ActionResult>;
  shutdown(waittime: number, message: string): Promise<ActionResult>;
  forceStop(): Promise<ActionResult>;
}

/** True when running inside the Tauri desktop webview (vs a plain browser). */
export function isTauri(): boolean {
  if (typeof window === "undefined") return false;
  return "__TAURI_INTERNALS__" in window || "__TAURI__" in window || "__TAURI_METADATA__" in window;
}

let demoMode = false;
/** Switch between real server and built-in demo data. */
export function setDemoMode(v: boolean) {
  demoMode = v;
}
export function isDemoMode(): boolean {
  return demoMode;
}

/** The adapter that should handle calls right now. */
export function activeApi(): PalworldApi {
  if (demoMode) return mockApi;
  return isTauri() ? tauriApi : httpApi;
}

/** Stable facade: every call is delegated to the currently active adapter. */
export const api: PalworldApi = {
  testConnection: (c) => activeApi().testConnection(c),
  getInfo: () => activeApi().getInfo(),
  getMetrics: () => activeApi().getMetrics(),
  getPlayers: () => activeApi().getPlayers(),
  getSettings: () => activeApi().getSettings(),
  getGameData: () => activeApi().getGameData(),
  announce: (m) => activeApi().announce(m),
  kick: (u, m) => activeApi().kick(u, m),
  ban: (u, m) => activeApi().ban(u, m),
  unban: (u) => activeApi().unban(u),
  saveWorld: () => activeApi().saveWorld(),
  shutdown: (w, m) => activeApi().shutdown(w, m),
  forceStop: () => activeApi().forceStop(),
};
