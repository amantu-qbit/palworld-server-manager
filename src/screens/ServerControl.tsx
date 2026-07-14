import "./servercontrol.css";
import { useState } from "react";
import { Loader2, Play, RotateCw, ServerCog, Square, TriangleAlert } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { Button } from "../components/Button";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { ConfirmSpec } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { Skeleton } from "../components/Skeleton";
import { useBridge, useServerStatus } from "../hooks/bridge";
import { bridgeApi } from "../api/bridge";
import { api } from "../api";
import { useToast } from "../hooks/useToast";

/** Seconds of in-game warning before a graceful stop/restart. */
const GRACE_SECS = 5;

function fmtUptime(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h) return `${h}h ${m}m`;
  if (m) return `${m}m ${s}s`;
  return `${s}s`;
}

/** Poll `fn` until it resolves truthy or the timeout elapses. */
async function waitUntil(fn: () => Promise<boolean>, timeoutMs = 90_000, intervalMs = 1500): Promise<boolean> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      if (await fn()) return true;
    } catch {
      /* keep polling — transient errors while the server is coming up/down */
    }
    await new Promise((r) => setTimeout(r, intervalMs));
  }
  return false;
}

export function ServerControl() {
  const bridge = useBridge();
  const status = useServerStatus(bridge.available);
  const toast = useToast();
  const [confirm, setConfirm] = useState<ConfirmSpec | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const data = status.data;
  const running = !!data?.running;

  async function run(label: string, fn: () => Promise<void>) {
    setBusy(label);
    try {
      await fn();
    } catch (e) {
      toast.error("Server control failed", e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
      status.refetch();
    }
  }

  const start = () =>
    run("start", async () => {
      await bridgeApi.serverStart();
      toast.success("Server starting", "PalServer.exe launched.");
    });

  const gracefulStop = () =>
    setConfirm({
      title: "Stop the server?",
      body: `Players get a ${GRACE_SECS}s warning, the world saves, then the server stops.`,
      confirmText: "Stop server",
      onConfirm: () =>
        run("stop", async () => {
          const r = await api.shutdown(GRACE_SECS, "Server shutting down.");
          if (!r.ok) throw new Error(r.message ?? "Shutdown request failed.");
          await waitUntil(() => bridgeApi.serverStatus().then((s) => !s.running));
          toast.success("Server stopped", "The world was saved on shutdown.");
        }),
    });

  const gracefulRestart = () =>
    setConfirm({
      title: "Restart the server?",
      body: `Players get a ${GRACE_SECS}s warning and the world saves, then the server relaunches.`,
      confirmText: "Restart server",
      onConfirm: () =>
        run("restart", async () => {
          const r = await api.shutdown(GRACE_SECS, "Server restarting.");
          if (!r.ok) throw new Error(r.message ?? "Shutdown request failed.");
          const stopped = await waitUntil(() => bridgeApi.serverStatus().then((s) => !s.running));
          if (!stopped) throw new Error("Server didn't stop in time; not relaunching.");
          await bridgeApi.serverStart();
          await waitUntil(() => bridgeApi.serverStatus().then((s) => s.running), 30_000);
          toast.success("Server restarted", "Back up and running.");
        }),
    });

  const forceStop = () =>
    setConfirm({
      title: "Force-stop the server?",
      body: "The process is killed immediately — no player warning, no shutdown save. Use only if the server is hung.",
      confirmText: "Force stop",
      danger: true,
      onConfirm: () =>
        run("force-stop", async () => {
          await bridgeApi.serverStop();
          toast.success("Server force-stopped", "Process killed.");
        }),
    });

  const forceRestart = () =>
    setConfirm({
      title: "Force-restart the server?",
      body: "The process is killed immediately (no save) and relaunched. Use only if the server is hung.",
      confirmText: "Force restart",
      danger: true,
      onConfirm: () =>
        run("force-restart", async () => {
          await bridgeApi.serverRestart();
          toast.success("Server restarted", "Process killed and relaunched.");
        }),
    });

  return (
    <>
      <TopBar
        breadcrumb="Server+"
        title="Server Control"
        onRefresh={() => status.refetch()}
        refreshing={status.isFetching}
      />
      <div className="page">
        {status.isLoading && !data ? (
          <div className="card card--pad col" style={{ gap: 12 }}>
            <Skeleton height={82} radius="var(--r-md)" />
            <Skeleton height={44} radius="var(--r-md)" />
          </div>
        ) : status.isError ? (
          <EmptyState
            icon={TriangleAlert}
            tone="error"
            title="Can’t reach the bridge"
            detail={(status.error as Error)?.message ?? "The bridge didn’t respond."}
          />
        ) : data && !data.configured ? (
          <NotConfigured />
        ) : data ? (
          <div className="sc">
            <div className={`sc-status card card--pad ${running ? "is-up" : "is-down"}`}>
              <span className="sc-status__dot" />
              <div className="sc-status__main">
                <b>{running ? "Running" : "Stopped"}</b>
                <small>
                  {running
                    ? `PID ${data.pid ?? "?"} · up ${fmtUptime(data.uptime_secs ?? 0)}`
                    : "The server process is not running."}
                </small>
              </div>
              {busy && <Loader2 size={18} className="spin sc-status__busy" />}
            </div>

            <div className="sc-actions">
              {running ? (
                <>
                  <Button variant="primary" onClick={gracefulRestart} disabled={!!busy} loading={busy === "restart"}>
                    <RotateCw size={16} /> Restart
                  </Button>
                  <Button variant="ghost" onClick={gracefulStop} disabled={!!busy} loading={busy === "stop"}>
                    <Square size={16} /> Stop
                  </Button>
                </>
              ) : (
                <Button variant="primary" onClick={start} disabled={!!busy} loading={busy === "start"}>
                  <Play size={16} /> Start server
                </Button>
              )}
            </div>

            <p className="sc-hint">
              Graceful actions warn players and save the world through Palworld’s own shutdown, then the bridge
              relaunches the process. Start the server from here so the bridge supervises it.
            </p>

            {running && (
              <div className="sc-force">
                <div className="eyebrow">If the server is hung</div>
                <div className="sc-actions">
                  <Button
                    variant="ghost"
                    onClick={forceRestart}
                    disabled={!!busy}
                    loading={busy === "force-restart"}
                  >
                    Force restart
                  </Button>
                  <Button variant="danger" onClick={forceStop} disabled={!!busy} loading={busy === "force-stop"}>
                    Force stop
                  </Button>
                </div>
                <p className="sc-hint sc-hint--warn">Force actions kill the process immediately — no save.</p>
              </div>
            )}
          </div>
        ) : null}
      </div>

      <ConfirmDialog spec={confirm} onClose={() => setConfirm(null)} />
    </>
  );
}

function NotConfigured() {
  return (
    <div className="card card--pad sc-setup">
      <ServerCog size={22} />
      <h3>Server control isn’t set up yet</h3>
      <p>
        Open <span className="mono">psm-bridge.exe</span> on your server, set the{" "}
        <b>Server executable</b> (and any launch args) in its window, and click <b>Save</b>. The
        Start / Stop / Restart controls appear here once it’s configured.
      </p>
      <p className="sc-hint">
        Prefer editing the file? Add a <span className="mono">[server_process]</span> section to{" "}
        <span className="mono">bridge.toml</span> instead:
      </p>
      <pre className="sc-toml">{`[server_process]
exe = "E:/SteamLibrary/steamapps/common/PalServer/PalServer.exe"
args = ["-useperfthreads", "-NoAsyncLoadingThread"]`}</pre>
    </div>
  );
}
