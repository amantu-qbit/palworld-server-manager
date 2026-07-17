import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  clearHistory,
  historySnapshot,
  recentSnapshot,
  recordMetrics,
  setActiveServer,
} from "./metricsHistory";
import type { Metrics } from "../types/api";

const sample = (over: Partial<Metrics> = {}): Metrics => ({
  serverfps: 60,
  currentplayernum: 2,
  serverframetime: 16,
  maxplayernum: 32,
  uptime: 100,
  ...over,
});

// The vitest env is "node" — install a fresh in-memory localStorage before each
// test (any partial global stub is replaced) so the per-server persistence path
// is exercised and isolated between tests.
function ensureLocalStorage() {
  const mem = new Map<string, string>();
  (globalThis as unknown as { localStorage: Storage }).localStorage = {
    getItem: (k: string) => (mem.has(k) ? mem.get(k)! : null),
    setItem: (k: string, v: string) => void mem.set(k, String(v)),
    removeItem: (k: string) => void mem.delete(k),
    clear: () => mem.clear(),
    key: (i: number) => [...mem.keys()][i] ?? null,
    get length() {
      return mem.size;
    },
  } as Storage;
}

// Each test starts from a clean module state on a known server + fixed clock.
beforeEach(() => {
  ensureLocalStorage();
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
  localStorage.clear();
  // Force a server switch away-then-back so the module reloads a clean slate.
  setActiveServer("reset", 0);
  clearHistory();
  setActiveServer("host", 8212);
  clearHistory();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("metricsHistory", () => {
  it("averages fps/frame and keeps the latest players/uptime within one minute", () => {
    recordMetrics(sample({ serverfps: 60, serverframetime: 10, currentplayernum: 1, uptime: 100 }));
    vi.advanceTimersByTime(3000);
    recordMetrics(sample({ serverfps: 40, serverframetime: 20, currentplayernum: 3, uptime: 103 }));

    const h = historySnapshot();
    expect(h).toHaveLength(1);
    expect(h[0].fps).toBe(50); // (60 + 40) / 2
    expect(h[0].frame).toBe(15); // (10 + 20) / 2
    expect(h[0].players).toBe(3); // latest
    expect(h[0].uptime).toBe(103); // latest
    expect(h[0].n).toBe(2);
  });

  it("starts a new bucket when the minute rolls over", () => {
    recordMetrics(sample({ serverfps: 60 }));
    vi.advanceTimersByTime(61_000); // cross a minute boundary
    recordMetrics(sample({ serverfps: 30 }));

    const h = historySnapshot();
    expect(h).toHaveLength(2);
    expect(h[0].fps).toBe(60);
    expect(h[1].fps).toBe(30);
    expect(h[1].n).toBe(1);
  });

  it("keeps raw samples in the recent buffer", () => {
    recordMetrics(sample({ serverfps: 60 }));
    vi.advanceTimersByTime(3000);
    recordMetrics(sample({ serverfps: 55 }));
    expect(recentSnapshot().map((s) => s.fps)).toEqual([60, 55]);
  });

  it("isolates history per server and persists to localStorage", () => {
    recordMetrics(sample({ serverfps: 60 }));
    expect(historySnapshot()).toHaveLength(1);

    setActiveServer("other", 9999); // switch servers → separate series
    expect(historySnapshot()).toHaveLength(0);

    setActiveServer("host", 8212); // switch back → reloaded from localStorage
    expect(historySnapshot()).toHaveLength(1);
    expect(historySnapshot()[0].fps).toBe(60);
  });

  it("clearHistory wipes both tiers", () => {
    recordMetrics(sample());
    clearHistory();
    expect(historySnapshot()).toHaveLength(0);
    expect(recentSnapshot()).toHaveLength(0);
  });
});
