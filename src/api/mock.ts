import type {
  ActionResult,
  Connection,
  GameData,
  Metrics,
  Player,
  ServerInfo,
  Settings,
} from "../types/api";
import type { PalworldApi } from "./index";
import * as fx from "./fixtures";

const bootedAt = Date.now();

function delay<T>(value: T, ms = 220): Promise<T> {
  return new Promise((resolve) => setTimeout(() => resolve(value), ms));
}

/** Small live-feeling jitter around a base value. */
function jitter(base: number, amp: number): number {
  return base + (Math.random() - 0.5) * 2 * amp;
}

/**
 * Mock implementation of the Palworld API. Read calls return the fixtures
 * (metrics/game-data drift slightly per call so the UI feels alive); action
 * calls always succeed and are echoed back.
 */
export const mockApi: PalworldApi = {
  async testConnection(c: Connection) {
    if (!c.host || !c.port) return { ok: false, message: "Host and port are required." };
    if (!c.password) return { ok: false, message: "Admin password is required." };
    return delay<ActionResult>({ ok: true, message: "Connected · 42ms" }, 500);
  },

  getInfo(): Promise<ServerInfo> {
    return delay(fx.serverInfo);
  },

  getMetrics(): Promise<Metrics> {
    const elapsed = Math.floor((Date.now() - bootedAt) / 1000);
    return delay<Metrics>({
      ...fx.baseMetrics,
      serverfps: Math.round(jitter(58.5, 2.5)),
      serverframetime: Number(jitter(16.9, 1.2).toFixed(1)),
      uptime: fx.baseMetrics.uptime + elapsed,
      currentplayernum: fx.players.length,
    });
  },

  getPlayers(): Promise<Player[]> {
    return delay(
      fx.players.map((p) => ({ ...p, ping: Math.max(6, Math.round(jitter(p.ping, 8))) })),
    );
  },

  getSettings(): Promise<Settings> {
    return delay(fx.settings);
  },

  getGameData(): Promise<GameData> {
    return delay<GameData>({
      ...fx.gameData,
      FPS: Math.round(jitter(58.5, 2.5)),
      AverageFPS: Number(jitter(58.4, 0.4).toFixed(1)),
    });
  },

  announce(message: string) {
    return delay<ActionResult>({ ok: true, message: `Announced: "${message}"` }, 350);
  },
  kick(userid: string) {
    return delay<ActionResult>({ ok: true, message: `Kicked ${userid}` }, 350);
  },
  ban(userid: string) {
    return delay<ActionResult>({ ok: true, message: `Banned ${userid}` }, 350);
  },
  unban(userid: string) {
    return delay<ActionResult>({ ok: true, message: `Unbanned ${userid}` }, 350);
  },
  saveWorld() {
    return delay<ActionResult>({ ok: true, message: "World saved." }, 600);
  },
  shutdown(waittime: number, message: string) {
    return delay<ActionResult>(
      { ok: true, message: `Shutdown in ${waittime}s — "${message}"` },
      350,
    );
  },
  forceStop() {
    return delay<ActionResult>({ ok: true, message: "Server stopping." }, 350);
  },
};
