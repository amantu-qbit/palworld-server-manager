import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "./index";
import type { Connection } from "../types/api";
import type {
  BridgeHealth,
  ContainerInfo,
  ContainersResponse,
  ContainerWriteResult,
  EditPalBody,
  EditPlayerBody,
  EditPlayerTechnologiesBody,
  Guild,
  PlayerDetail,
  PlayerSummary,
  ReferenceCatalog,
  SavFileInfo,
  SavTreeResponse,
  ServerStatus,
  WriteResult,
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
  /** Every labeled item container (player bags + guild chests). */
  containers(): Promise<ContainerInfo[]>;
  /** Resize a container; shrinking deletes slots `>= slotNum` (backup taken). */
  resizeContainer(cid: string, slotNum: number): Promise<ContainerWriteResult>;
  /** Write one slot; `staticId: "None"` or `count: 0` clears it. */
  setContainerSlot(
    cid: string,
    slotIndex: number,
    staticId: string,
    count: number,
  ): Promise<ContainerWriteResult>;
  /** Remove every occupied slot in one write (a single backup). */
  clearContainer(cid: string): Promise<ContainerWriteResult>;
  /** Edit player level/EXP/status points (Level.sav). */
  editPlayer(uid: string, body: EditPlayerBody): Promise<WriteResult>;
  /** Unlock/relock technologies and set tech points (per-player `<UID>.sav`). */
  editPlayerTechnologies(uid: string, body: EditPlayerTechnologiesBody): Promise<WriteResult>;
  /** Edit one Pal instance (level, nickname, souls, talents, skills…). */
  editPal(instanceId: string, body: EditPalBody): Promise<WriteResult>;
}

const path = {
  playerDetail: (uid: string) => `/players/${encodeURIComponent(uid)}`,
  reference: (catalog: string) => `/reference/${encodeURIComponent(catalog)}`,
  resize: (cid: string) => `/containers/${encodeURIComponent(cid)}/resize`,
  slot: (cid: string) => `/containers/${encodeURIComponent(cid)}/slot`,
  clear: (cid: string) => `/containers/${encodeURIComponent(cid)}/clear`,
  editPlayer: (uid: string) => `/players/${encodeURIComponent(uid)}/edit`,
  technologies: (uid: string) => `/players/${encodeURIComponent(uid)}/technologies`,
  editPal: (id: string) => `/pals/${encodeURIComponent(id)}/edit`,
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
  containers: () =>
    invoke<ContainersResponse>("bridge_get", { path: "/containers" }).then((r) => r.containers),
  resizeContainer: (cid, slotNum) =>
    invoke<ContainerWriteResult>("bridge_post", { path: path.resize(cid), body: { slot_num: slotNum } }),
  setContainerSlot: (cid, slotIndex, staticId, count) =>
    invoke<ContainerWriteResult>("bridge_post", {
      path: path.slot(cid),
      body: { slot_index: slotIndex, static_id: staticId, count },
    }),
  clearContainer: (cid) => invoke<ContainerWriteResult>("bridge_post", { path: path.clear(cid) }),
  editPlayer: (uid, body) => invoke<WriteResult>("bridge_post", { path: path.editPlayer(uid), body }),
  editPlayerTechnologies: (uid, body) =>
    invoke<WriteResult>("bridge_post", { path: path.technologies(uid), body }),
  editPal: (instanceId, body) =>
    invoke<WriteResult>("bridge_post", { path: path.editPal(instanceId), body }),
};

let conn: Connection | null = null;

async function call<T>(p: string, method = "GET", body?: unknown): Promise<T> {
  if (!conn?.bridgePort || !conn?.bridgeToken) throw new Error("Bridge not configured.");
  const headers: Record<string, string> = {
    "x-bridge-host": conn.host,
    "x-bridge-port": String(conn.bridgePort),
    "x-bridge-token": conn.bridgeToken,
  };
  if (body !== undefined) headers["content-type"] = "application/json";
  const res = await fetch(`/__bridge__${p}`, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
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
      const parsed = JSON.parse(text) as { detail?: string; error?: string };
      detail = parsed?.detail ?? parsed?.error ?? "";
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
  containers: () => call<ContainersResponse>("/containers").then((r) => r.containers),
  resizeContainer: (cid, slotNum) =>
    call<ContainerWriteResult>(path.resize(cid), "POST", { slot_num: slotNum }),
  setContainerSlot: (cid, slotIndex, staticId, count) =>
    call<ContainerWriteResult>(path.slot(cid), "POST", {
      slot_index: slotIndex,
      static_id: staticId,
      count,
    }),
  clearContainer: (cid) => call<ContainerWriteResult>(path.clear(cid), "POST"),
  editPlayer: (uid, body) => call<WriteResult>(path.editPlayer(uid), "POST", body),
  editPlayerTechnologies: (uid, body) => call<WriteResult>(path.technologies(uid), "POST", body),
  editPal: (instanceId, body) => call<WriteResult>(path.editPal(instanceId), "POST", body),
};

export const bridgeApi: BridgeApi = isTauri() ? tauriBridge : httpBridge;
