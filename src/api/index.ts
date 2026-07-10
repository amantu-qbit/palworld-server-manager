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

/**
 * The single surface every screen talks to. Two implementations exist:
 *  - `tauriApi`  — real; calls Rust commands that perform HTTP Basic-auth requests.
 *  - `mockApi`   — realistic fixtures; powers browser dev/preview with no backend.
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

/** True when running inside the Tauri webview (vs a plain browser). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Pick the real backend inside the app, the mock everywhere else. */
export function selectApi(): PalworldApi {
  return isTauri() ? tauriApi : mockApi;
}

export const api = selectApi();
