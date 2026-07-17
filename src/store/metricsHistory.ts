import { useSyncExternalStore } from "react";
import type { Metrics } from "../types/api";

/**
 * Persistent, per-server metrics history for the Trends screen.
 *
 * The app can only sample while it's open, connected, and not paused, so the
 * series is inherently gappy — we store real samples only (no synthetic fill)
 * and let consumers draw gaps as breaks. Mirrors the localStorage ring-buffer
 * pattern of `activityLog.ts` / `banRecord.ts`.
 *
 * Two tiers:
 *  - `history` — one **minutely** aggregate per minute (avg fps/frame, latest
 *    players/uptime), capped at 24h, persisted per server. Long trend view.
 *  - `recent`  — the last raw per-poll samples, in memory only, for the small
 *    live sparklines on the Dashboard (seeded from persisted history on load so
 *    they're never blank, but never persisted themselves).
 */
export interface MetricSample {
  /** Epoch ms. Minute-truncated in `history`, raw in `recent`. */
  t: number;
  fps: number;
  players: number;
  /** Frame time, ms. */
  frame: number;
  /** maxplayernum at the sample. */
  max: number;
  /** Server uptime, seconds (monotonic; a drop marks a restart). */
  uptime: number;
  /** Samples folded into this minute bucket (history only). */
  n?: number;
}

const MINUTE = 60_000;
const HISTORY_CAP = 1440; // 24h at 1 sample/min
const RECENT_CAP = 120;
const keyFor = (server: string) => `psm.metrics.${server}`;

let activeServer = "";
let history: MetricSample[] = [];
let recent: MetricSample[] = [];
const listeners = new Set<() => void>();

function loadHistory(server: string): MetricSample[] {
  try {
    const raw = localStorage.getItem(keyFor(server));
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as MetricSample[]) : [];
  } catch {
    return [];
  }
}

function persist(): void {
  if (!activeServer) return;
  try {
    localStorage.setItem(keyFor(activeServer), JSON.stringify(history));
  } catch {
    // Best-effort — ignore quota / private-mode failures.
  }
}

function emit(): void {
  for (const l of listeners) l();
}

/**
 * Switch the history to a given server (host:port). Loads that server's stored
 * series and seeds the live buffer from its tail so charts aren't blank on
 * connect. A no-op if the server is unchanged.
 */
export function setActiveServer(host: string, port: number): void {
  const server = `${host}:${port}`;
  if (server === activeServer) return;
  activeServer = server;
  history = loadHistory(server);
  recent = history.slice(-RECENT_CAP);
  emit();
}

/** Record one live metrics sample into both tiers. No-op until a server is set. */
export function recordMetrics(m: Metrics): void {
  if (!activeServer) return;
  const now = Date.now();
  const raw: MetricSample = {
    t: now,
    fps: m.serverfps,
    players: m.currentplayernum,
    frame: m.serverframetime,
    max: m.maxplayernum,
    uptime: m.uptime,
  };
  recent = [...recent, raw].slice(-RECENT_CAP);

  const minute = Math.floor(now / MINUTE) * MINUTE;
  const last = history[history.length - 1];
  if (last && last.t === minute) {
    // Fold into the in-progress minute; `n` (persisted) keeps averages exact
    // across reloads within the same minute.
    const prevN = last.n ?? 1;
    const n = prevN + 1;
    const merged: MetricSample = {
      t: minute,
      fps: (last.fps * prevN + m.serverfps) / n,
      frame: (last.frame * prevN + m.serverframetime) / n,
      players: m.currentplayernum,
      max: m.maxplayernum,
      uptime: m.uptime,
      n,
    };
    history = [...history.slice(0, -1), merged];
  } else {
    history = [
      ...history,
      {
        t: minute,
        fps: m.serverfps,
        frame: m.serverframetime,
        players: m.currentplayernum,
        max: m.maxplayernum,
        uptime: m.uptime,
        n: 1,
      },
    ].slice(-HISTORY_CAP);
  }
  persist();
  emit();
}

/** Wipe the active server's stored history (and live buffer). */
export function clearHistory(): void {
  history = [];
  recent = [];
  persist();
  emit();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Current persisted minutely history for the active server (oldest → newest). */
export function historySnapshot(): MetricSample[] {
  return history;
}

/** Current in-memory recent raw samples. */
export function recentSnapshot(): MetricSample[] {
  return recent;
}

/** Persisted minutely history for the active server (oldest → newest). */
export function useMetricsHistory(): MetricSample[] {
  return useSyncExternalStore(subscribe, historySnapshot, historySnapshot);
}

/** Recent raw per-poll samples for live sparklines (in-memory). */
export function useRecentMetrics(): MetricSample[] {
  return useSyncExternalStore(subscribe, recentSnapshot, recentSnapshot);
}
