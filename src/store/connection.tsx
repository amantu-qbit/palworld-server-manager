import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Connection } from "../types/api";
import { api, isTauri, setDemoMode } from "../api";

const STORAGE_KEY = "psm.connection";

interface ConnectionState {
  connection: Connection | null;
  connected: boolean;
  connecting: boolean;
  /** True when the current session is showing built-in demo data. */
  demo: boolean;
  /** Test + establish a connection. Persists on success. Pass {demo:true} for sample data. */
  connect: (c: Connection, opts?: { demo?: boolean }) => Promise<{ ok: boolean; message?: string }>;
  /** Test without establishing (for the "Test connection" button). */
  test: (c: Connection) => Promise<{ ok: boolean; message?: string }>;
  disconnect: () => void;
  /** Last saved connection fields, to prefill the form. */
  remembered: Connection;
}

const DEFAULT_CONN: Connection = { host: "localhost", port: 8212, password: "" };

const Ctx = createContext<ConnectionState | null>(null);

function load(): Connection {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { ...DEFAULT_CONN, ...(JSON.parse(raw) as Partial<Connection>) };
  } catch {
    /* ignore */
  }
  return DEFAULT_CONN;
}

export function ConnectionProvider({ children }: { children: ReactNode }) {
  const [remembered, setRemembered] = useState<Connection>(load);
  const [connection, setConnection] = useState<Connection | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [demo, setDemo] = useState(false);

  const test = useCallback(async (c: Connection) => {
    setDemoMode(false);
    return api.testConnection(c);
  }, []);

  const connect = useCallback(async (c: Connection, opts?: { demo?: boolean }) => {
    const demoOn = !!opts?.demo;
    setConnecting(true);
    try {
      setDemoMode(demoOn);
      const res = await api.testConnection(c);
      if (!res.ok) {
        setDemoMode(false);
        return res;
      }
      // In the desktop app, hand real credentials to the Rust backend so
      // subsequent commands are authenticated.
      if (isTauri() && !demoOn) {
        await invoke("save_connection", { host: c.host, port: c.port, password: c.password });
      }
      setDemo(demoOn);
      setConnection(c);
      if (!demoOn) {
        setRemembered(c);
        try {
          localStorage.setItem(STORAGE_KEY, JSON.stringify(c));
        } catch {
          /* ignore */
        }
      }
      return { ok: true };
    } finally {
      setConnecting(false);
    }
  }, []);

  const disconnect = useCallback(() => {
    setDemoMode(false);
    setDemo(false);
    setConnection(null);
  }, []);

  // Keep the remembered fields in sync if another tab writes them.
  useEffect(() => {
    const onStorage = (e: StorageEvent) => {
      if (e.key === STORAGE_KEY) setRemembered(load());
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  const value = useMemo<ConnectionState>(
    () => ({
      connection,
      connected: connection !== null,
      connecting,
      demo,
      connect,
      test,
      disconnect,
      remembered,
    }),
    [connection, connecting, demo, connect, test, disconnect, remembered],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useConnection(): ConnectionState {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useConnection must be used within ConnectionProvider");
  return ctx;
}
