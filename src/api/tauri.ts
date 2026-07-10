import { invoke } from "@tauri-apps/api/core";
import type {
  ActionResult,
  Connection,
  GameData,
  Metrics,
  Player,
  PlayersResponse,
  ServerInfo,
  Settings,
} from "../types/api";
import type { PalworldApi } from "./index";

/** Normalize a thrown Rust error (string) into a readable message. */
function toMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return "Request failed.";
}

async function ok(fn: () => Promise<unknown>): Promise<ActionResult> {
  try {
    await fn();
    return { ok: true };
  } catch (err) {
    return { ok: false, message: toMessage(err) };
  }
}

/**
 * Real implementation. Each method calls a Rust `#[tauri::command]` that
 * performs the HTTP Basic-auth request server-side (no browser CORS).
 */
export const tauriApi: PalworldApi = {
  async testConnection(c: Connection) {
    try {
      await invoke("test_connection", { host: c.host, port: c.port, password: c.password });
      return { ok: true, message: "Connected" };
    } catch (err) {
      return { ok: false, message: toMessage(err) };
    }
  },

  getInfo() {
    return invoke<ServerInfo>("get_info");
  },
  getMetrics() {
    return invoke<Metrics>("get_metrics");
  },
  async getPlayers(): Promise<Player[]> {
    const res = await invoke<PlayersResponse>("get_players");
    return res.players ?? [];
  },
  getSettings() {
    return invoke<Settings>("get_settings");
  },
  getGameData() {
    return invoke<GameData>("get_game_data");
  },

  announce(message: string) {
    return ok(() => invoke("announce", { message }));
  },
  kick(userid: string, message: string) {
    return ok(() => invoke("kick", { userid, message }));
  },
  ban(userid: string, message: string) {
    return ok(() => invoke("ban", { userid, message }));
  },
  unban(userid: string) {
    return ok(() => invoke("unban", { userid }));
  },
  saveWorld() {
    return ok(() => invoke("save_world"));
  },
  shutdown(waittime: number, message: string) {
    return ok(() => invoke("shutdown", { waittime, message }));
  },
  forceStop() {
    return ok(() => invoke("force_stop"));
  },
};
