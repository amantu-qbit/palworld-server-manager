/** Human-friendly formatting helpers. */

/** Uptime in seconds → the two largest non-zero units, e.g. "4d 12h", "1m 30s". */
export function formatUptime(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${sec}s`;
  return `${sec}s`;
}

/** Split uptime into value + unit pairs for styled rendering. */
export function uptimeParts(totalSeconds: number): { value: number; unit: string }[] {
  const s = Math.max(0, Math.floor(totalSeconds));
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (d > 0) return [{ value: d, unit: "d" }, { value: h, unit: "h" }];
  if (h > 0) return [{ value: h, unit: "h" }, { value: m, unit: "m" }];
  if (m > 0) return [{ value: m, unit: "m" }, { value: sec, unit: "s" }];
  return [{ value: sec, unit: "s" }];
}

/** Thousands-separated integer. */
export function formatInt(n: number): string {
  return Math.round(n).toLocaleString("en-US");
}

/** One-decimal millisecond string. */
export function formatMs(n: number): string {
  return `${n.toFixed(1)}`;
}

/** Compact coordinate label (whole thousands). */
export function formatCoord(n: number): string {
  return Math.round(n).toLocaleString("en-US");
}

/** Convert a camelCase / bPrefixed setting key into a readable label. */
export function humanLabel(key: string): string {
  let k = key;
  if (/^b[A-Z]/.test(k)) k = k.slice(1); // drop boolean "b" prefix
  k = k.replace(/_/g, " ");
  k = k.replace(/([a-z0-9])([A-Z])/g, "$1 $2");
  k = k.replace(/\bUNKO\b/g, "UNKO").replace(/\bHP\b/g, "HP").replace(/\bIP\b/g, "IP");
  return k.replace(/\s+/g, " ").trim();
}
