import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "./index";
import type { Connection } from "../types/api";
import type {
  BridgeHealth,
  Guild,
  PlayerDetail,
  PlayerSummary,
  ReferenceCatalog,
  SavFileInfo,
  SavTreeResponse,
  ServerStatus,
} from "../types/bridge";

/**
 * The Tier-2 bridge surface. Independent of `PalworldApi` — it talks to
 * psm-bridge.exe over Bearer auth, and is only reachable when the owner has
 * configured a bridge port + token. Two implementations:
 *  - `tauriBridge` — a Rust `bridge_get` command does the request (desktop app).
 *  - `httpBridge`  — the browser build routes through the `/__bridge__` dev proxy.
 */
export interface BridgeApi {
  /** Hand bridge creds to the backend (tauri) or store them (browser). */
  configure(c: Connection): Promise<void>;
  health(): Promise<BridgeHealth>;
  players(): Promise<PlayerSummary[]>;
  playerDetail(uid: string): Promise<PlayerDetail>;
  guilds(): Promise<Guild[]>;
  reference(catalog: string): Promise<ReferenceCatalog>;
  serverStatus(): Promise<ServerStatus>;
  serverStart(): Promise<ServerStatus>;
  serverStop(): Promise<ServerStatus>;
  serverRestart(): Promise<ServerStatus>;
  /** Raw Save debug: list `.sav` files under the bridge save dir. */
  savFiles(): Promise<SavFileInfo[]>;
  /** Raw Save debug: one bounded subtree of a `.sav`'s decoded GVAS tree. */
  savTree(file: string, path?: string, page?: number, depth?: number): Promise<SavTreeResponse>;
}

const path = {
  playerDetail: (uid: string) => `/players/${encodeURIComponent(uid)}`,
  reference: (catalog: string) => `/reference/${encodeURIComponent(catalog)}`,
  savTree: (file: string, sub = "", page?: number, depth?: number) => {
    const q = new URLSearchParams({ file });
    if (sub) q.set("path", sub);
    if (page != null) q.set("page", String(page));
    if (depth != null) q.set("depth", String(depth));
    return `/debug/savtree?${q.toString()}`;
  },
};

const tauriBridge: BridgeApi = {
  async configure(c) {
    if (c.bridgePort && c.bridgeToken) {
      await invoke("save_bridge", { host: c.host, port: c.bridgePort, token: c.bridgeToken });
    } else {
      await invoke("clear_bridge");
    }
  },
  health: () => invoke<BridgeHealth>("bridge_get", { path: "/health" }),
  players: () => invoke<PlayerSummary[]>("bridge_get", { path: "/players" }),
  playerDetail: (uid) => invoke<PlayerDetail>("bridge_get", { path: path.playerDetail(uid) }),
  guilds: () => invoke<Guild[]>("bridge_get", { path: "/guilds" }),
  reference: (catalog) => invoke<ReferenceCatalog>("bridge_get", { path: path.reference(catalog) }),
  serverStatus: () => invoke<ServerStatus>("bridge_get", { path: "/server/status" }),
  serverStart: () => invoke<ServerStatus>("bridge_post", { path: "/server/start" }),
  serverStop: () => invoke<ServerStatus>("bridge_post", { path: "/server/stop" }),
  serverRestart: () => invoke<ServerStatus>("bridge_post", { path: "/server/restart" }),
  savFiles: () => invoke<SavFileInfo[]>("bridge_get", { path: "/debug/savfiles" }),
  savTree: (file, sub, page, depth) =>
    invoke<SavTreeResponse>("bridge_get", { path: path.savTree(file, sub, page, depth) }),
};

let conn: Connection | null = null;

async function call<T>(p: string, method = "GET"): Promise<T> {
  if (!conn?.bridgePort || !conn?.bridgeToken) throw new Error("Bridge not configured.");
  const res = await fetch(`/__bridge__${p}`, {
    method,
    headers: {
      "x-bridge-host": conn.host,
      "x-bridge-port": String(conn.bridgePort),
      "x-bridge-token": conn.bridgeToken,
    },
  }).catch(() => {
    throw new Error("Could not reach the app proxy. Are you running `npm run dev`?");
  });

  if (res.status === 401) throw new Error("Bridge authentication failed. Check the bridge token.");
  if (res.status === 502 || res.status === 504) {
    throw new Error("Could not reach the bridge. Is psm-bridge.exe running on the server?");
  }
  const text = await res.text();
  if (!res.ok) {
    let detail = "";
    try {
      detail = (JSON.parse(text) as { detail?: string })?.detail ?? "";
    } catch {
      /* body wasn't JSON */
    }
    throw new Error(detail || `Bridge returned ${res.status}`);
  }
  return (text ? JSON.parse(text) : {}) as T;
}

const httpBridge: BridgeApi = {
  async configure(c) {
    conn = c;
  },
  health: () => call<BridgeHealth>("/health"),
  players: () => call<PlayerSummary[]>("/players"),
  playerDetail: (uid) => call<PlayerDetail>(path.playerDetail(uid)),
  guilds: () => call<Guild[]>("/guilds"),
  reference: (catalog) => call<ReferenceCatalog>(path.reference(catalog)),
  serverStatus: () => call<ServerStatus>("/server/status"),
  serverStart: () => call<ServerStatus>("/server/start", "POST"),
  serverStop: () => call<ServerStatus>("/server/stop", "POST"),
  serverRestart: () => call<ServerStatus>("/server/restart", "POST"),
  savFiles: () => call<SavFileInfo[]>("/debug/savfiles"),
  savTree: (file, sub, page, depth) => call<SavTreeResponse>(path.savTree(file, sub, page, depth)),
};

export const bridgeApi: BridgeApi = isTauri() ? tauriBridge : httpBridge;
