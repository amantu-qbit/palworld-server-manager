import { createContext, useContext, useMemo, useState } from "react";
import type { ReactNode } from "react";

interface Prefs {
  /** Auto-refresh interval for live queries, in ms. */
  pollInterval: number;
  setPollInterval: (ms: number) => void;
  /** When true, live polling is paused. */
  paused: boolean;
  setPaused: (p: boolean) => void;
}

export const POLL_OPTIONS = [
  { label: "2s", value: 2000 },
  { label: "3s", value: 3000 },
  { label: "5s", value: 5000 },
  { label: "10s", value: 10000 },
];

const Ctx = createContext<Prefs | null>(null);

export function PrefsProvider({ children }: { children: ReactNode }) {
  const [pollInterval, setPollInterval] = useState(3000);
  const [paused, setPaused] = useState(false);
  const value = useMemo(
    () => ({ pollInterval, setPollInterval, paused, setPaused }),
    [pollInterval, paused],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function usePrefs(): Prefs {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("usePrefs must be used within PrefsProvider");
  return ctx;
}
