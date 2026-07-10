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

/**
 * Real API for the browser build. The Palworld REST API can't be called directly
 * from a browser (CORS + HTTP Basic preflight), so requests go to the Vite dev
 * proxy at `/__palapi__`, which forwards them to the configured server with the
 * Basic-auth header attached. In the Tauri desktop app the Rust adapter is used
 * instead (see tauri.ts).
 */

let conn: Connection | null = null;

function toMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  return "Request failed.";
}

async function call<T>(path: string, method = "GET", body?: unknown): Promise<T> {
  if (!conn) throw new Error("Not connected.");
  const res = await fetch(`/__palapi__${path}`, {
    method,
    headers: {
      "x-pal-host": conn.host,
      "x-pal-port": String(conn.port),
      "x-pal-pass": conn.password,
      ...(body ? { "content-type": "application/json" } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  }).catch(() => {
    throw new Error("Could not reach the app proxy. Are you running `npm run dev`?");
  });

  if (res.status === 401) throw new Error("Authentication failed. Check the admin password.");
  if (res.status === 502 || res.status === 504) {
    throw new Error("Could not reach the server. Is it running and is the REST API enabled?");
  }
  if (!res.ok) throw new Error(`Server returned ${res.status}`);
  const text = await res.text();
  return (text ? JSON.parse(text) : {}) as T;
}

async function ok(fn: () => Promise<unknown>): Promise<ActionResult> {
  try {
    await fn();
    return { ok: true };
  } catch (err) {
    return { ok: false, message: toMessage(err) };
  }
}

export const httpApi: PalworldApi = {
  async testConnection(c: Connection) {
    conn = c;
    try {
      await call("/info");
      return { ok: true, message: "Connected" };
    } catch (err) {
      return { ok: false, message: toMessage(err) };
    }
  },
  getInfo: () => call<ServerInfo>("/info"),
  getMetrics: () => call<Metrics>("/metrics"),
  async getPlayers(): Promise<Player[]> {
    const res = await call<PlayersResponse>("/players");
    return res.players ?? [];
  },
  getSettings: () => call<Settings>("/settings"),
  getGameData: () => call<GameData>("/game-data"),
  announce: (message) => ok(() => call("/announce", "POST", { message })),
  kick: (userid, message) => ok(() => call("/kick", "POST", { userid, message })),
  ban: (userid, message) => ok(() => call("/ban", "POST", { userid, message })),
  unban: (userid) => ok(() => call("/unban", "POST", { userid })),
  saveWorld: () => ok(() => call("/save", "POST", {})),
  shutdown: (waittime, message) => ok(() => call("/shutdown", "POST", { waittime, message })),
  forceStop: () => ok(() => call("/stop", "POST", {})),
};
