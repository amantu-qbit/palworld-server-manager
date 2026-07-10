import type {
  ActionResult,
  Connection,
  GameData,
  Metrics,
  Player,
  ServerInfo,
  Settings,
} from "../types/api";
import { tauriApi } from "./tauri";
import { httpApi } from "./http";

/**
 * The single surface every screen talks to. Two real implementations exist:
 *  - `tauriApi` — Rust commands do the HTTP Basic-auth request (desktop app).
 *  - `httpApi`  — browser build talks to a real server via the Vite dev proxy.
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

export const api: PalworldApi = isTauri() ? tauriApi : httpApi;
