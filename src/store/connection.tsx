import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Connection } from "../types/api";
import { api, isTauri } from "../api";
import { queryClient } from "../hooks/queries";

const STORAGE_KEY = "psm.connection";

interface ConnectionState {
  connection: Connection | null;
  connected: boolean;
  connecting: boolean;
  /** Test + establish a connection. Persists on success. */
  connect: (c: Connection) => Promise<{ ok: boolean; message?: string }>;
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

  const test = useCallback((c: Connection) => api.testConnection(c), []);

  const connect = useCallback(async (c: Connection) => {
    setConnecting(true);
    try {
      const res = await api.testConnection(c);
      if (!res.ok) return res;
      // In the desktop app, hand credentials to the Rust backend so subsequent
      // commands are authenticated.
      if (isTauri()) {
        await invoke("save_connection", { host: c.host, port: c.port, password: c.password });
      }
      queryClient.clear(); // drop any prior data so screens load fresh
      setConnection(c);
      setRemembered(c);
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(c));
      } catch {
        /* ignore */
      }
      return { ok: true };
    } finally {
      setConnecting(false);
    }
  }, []);

  const disconnect = useCallback(() => {
    setConnection(null);
    queryClient.clear();
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
      connect,
      test,
      disconnect,
      remembered,
    }),
    [connection, connecting, connect, test, disconnect, remembered],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useConnection(): ConnectionState {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useConnection must be used within ConnectionProvider");
  return ctx;
}
