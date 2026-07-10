import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import type { DownloadEvent, Update } from "@tauri-apps/plugin-updater";
import { isTauri } from "../api";

type Status = "idle" | "checking" | "uptodate" | "available" | "downloading" | "installing" | "error";

interface UpdaterState {
  status: Status;
  /** An update is available or currently being applied. */
  available: boolean;
  version?: string;
  currentVersion?: string;
  notes?: string;
  /** Download progress, 0..1. */
  progress: number;
  error?: string;
  dismissed: boolean;
  /** False in the browser dev build — updates only work in the installed desktop app. */
  supported: boolean;
  check: (manual?: boolean) => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

const Ctx = createContext<UpdaterState | null>(null);

export function UpdaterProvider({ children }: { children: ReactNode }) {
  const supported = isTauri();
  const [status, setStatus] = useState<Status>("idle");
  const [version, setVersion] = useState<string>();
  const [currentVersion, setCurrentVersion] = useState<string>();
  const [notes, setNotes] = useState<string>();
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState<string>();
  const [dismissed, setDismissed] = useState(false);
  const updateRef = useRef<Update | null>(null);
  const didCheck = useRef(false);

  const check = useCallback(
    async (manual = false) => {
      if (!supported) {
        if (manual) {
          setStatus("error");
          setError("Updates are only available in the installed desktop app.");
        }
        return;
      }
      setStatus("checking");
      setError(undefined);
      try {
        const updater = await import("@tauri-apps/plugin-updater");
        const update = await updater.check();
        if (update) {
          updateRef.current = update;
          setVersion(update.version);
          setCurrentVersion(update.currentVersion);
          setNotes(update.body?.trim() || undefined);
          setDismissed(false);
          setStatus("available");
        } else {
          updateRef.current = null;
          setStatus("uptodate");
        }
      } catch (e) {
        setStatus("error");
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [supported],
  );

  const install = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;
    setStatus("downloading");
    setProgress(0);
    let total = 0;
    let received = 0;
    try {
      await update.downloadAndInstall((ev: DownloadEvent) => {
        if (ev.event === "Started") {
          total = ev.data.contentLength ?? 0;
        } else if (ev.event === "Progress") {
          received += ev.data.chunkLength;
          setProgress(total ? Math.min(1, received / total) : 0);
        } else if (ev.event === "Finished") {
          setProgress(1);
          setStatus("installing");
        }
      });
      const process = await import("@tauri-apps/plugin-process");
      await process.relaunch();
    } catch (e) {
      setStatus("error");
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const dismiss = useCallback(() => setDismissed(true), []);

  // One quiet check shortly after launch (desktop only).
  useEffect(() => {
    if (didCheck.current) return;
    didCheck.current = true;
    if (supported) void check(false);
  }, [supported, check]);

  const value = useMemo<UpdaterState>(
    () => ({
      status,
      available: status === "available" || status === "downloading" || status === "installing",
      version,
      currentVersion,
      notes,
      progress,
      error,
      dismissed,
      supported,
      check,
      install,
      dismiss,
    }),
    [status, version, currentVersion, notes, progress, error, dismissed, supported, check, install, dismiss],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useUpdater(): UpdaterState {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useUpdater must be used within UpdaterProvider");
  return ctx;
}
