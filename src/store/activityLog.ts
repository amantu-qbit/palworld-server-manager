import { useSyncExternalStore } from "react";

export interface LogEntry {
  id: number;
  ts: number;
  kind: "info" | "success" | "error";
  text: string;
}

const KEY = "psm.activity";
const CAP = 120;

function load(): LogEntry[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as LogEntry[]) : [];
  } catch {
    return [];
  }
}

/** Newest-first, capped at CAP. Reassigned (never mutated) so the reference is a stable snapshot. */
let entries: LogEntry[] = load();
let counter = entries.reduce((max, e) => Math.max(max, e.id ?? 0), 0);
const listeners = new Set<() => void>();

function persist(): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(entries));
  } catch {
    // Ignore quota / private-mode write failures — the log is best-effort.
  }
}

/** Prepend an entry, persist to localStorage, and notify subscribers. */
export function logActivity(kind: LogEntry["kind"], text: string): void {
  const entry: LogEntry = { id: ++counter, ts: Date.now(), kind, text };
  entries = [entry, ...entries].slice(0, CAP);
  persist();
  for (const l of listeners) l();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot(): LogEntry[] {
  return entries;
}

/** React binding — returns the newest-first entries, re-rendering on every log. */
export function useActivityLog(): LogEntry[] {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
