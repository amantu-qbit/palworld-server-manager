import { useSyncExternalStore } from "react";

/** A ban issued through this app. The REST API can't list bans, so we track them locally. */
export interface BanRecord {
  userid: string;
  reason: string;
  ts: number;
}

const KEY = "psm.bans";
const CAP = 200;

let records: BanRecord[] = load();
const listeners = new Set<() => void>();

function load(): BanRecord[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter(
        (r): r is BanRecord =>
          !!r &&
          typeof r.userid === "string" &&
          typeof r.reason === "string" &&
          typeof r.ts === "number",
      )
      .slice(0, CAP);
  } catch {
    return [];
  }
}

function emit(): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(records));
  } catch {
    // storage unavailable / over quota — keep the in-memory copy anyway
  }
  listeners.forEach((l) => l());
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Record a ban (newest first, de-duped by user id, capped). */
export function addBan(userid: string, reason: string): void {
  records = [{ userid, reason, ts: Date.now() }, ...records.filter((r) => r.userid !== userid)].slice(
    0,
    CAP,
  );
  emit();
}

/** Forget a recorded ban by user id. */
export function removeBan(userid: string): void {
  const next = records.filter((r) => r.userid !== userid);
  if (next.length === records.length) return;
  records = next;
  emit();
}

/** Recorded bans, newest first. */
export function useBans(): BanRecord[] {
  return useSyncExternalStore(
    subscribe,
    () => records,
    () => records,
  );
}
