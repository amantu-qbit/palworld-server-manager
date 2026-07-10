import { useEffect, useState } from "react";
import { Activity, Clock, Megaphone, Power, Save, Timer, TriangleAlert, Users, Zap } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { Gauge } from "../components/Gauge";
import { MetricTile } from "../components/MetricTile";
import { StatusPill } from "../components/StatusPill";
import { Skeleton } from "../components/Skeleton";
import { EmptyState } from "../components/EmptyState";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { ConfirmSpec } from "../components/ConfirmDialog";
import { useInfo, useMetrics, useSettings } from "../hooks/queries";
import { formatMs, formatUptime } from "../lib/format";
import { api } from "../api";
import { useToast } from "../hooks/useToast";
import { useNav } from "../store/nav";

interface Hist {
  fps: number[];
  players: number[];
  frame: number[];
}

export function Dashboard() {
  const info = useInfo();
  const metrics = useMetrics();
  const settings = useSettings();
  const toast = useToast();
  const { navigate } = useNav();
  const [confirm, setConfirm] = useState<ConfirmSpec | null>(null);
  const [hist, setHist] = useState<Hist>({ fps: [], players: [], frame: [] });

  const m = metrics.data;

  useEffect(() => {
    if (!m) return;
    setHist((h) => {
      if (h.fps.length === 0) {
        const seed = (base: number, amp: number) =>
          Array.from({ length: 12 }, (_, i) => Math.round((base + Math.sin(i / 2) * amp) * 10) / 10);
        return { fps: seed(m.serverfps, 2), players: seed(m.currentplayernum, 1), frame: seed(m.serverframetime, 1) };
      }
      const push = (a: number[], v: number) => [...a, v].slice(-24);
      return {
        fps: push(h.fps, m.serverfps),
        players: push(h.players, m.currentplayernum),
        frame: push(h.frame, m.serverframetime),
      };
    });
  }, [m]);

  const s = settings.data;
  const healthy = (m?.serverfps ?? 0) >= 45;

  return (
    <>
      <TopBar breadcrumb="Overview" title="Dashboard" onRefresh={() => metrics.refetch()} refreshing={metrics.isFetching} />
      <div className="page">
        {metrics.isError ? (
          <EmptyState
            icon={TriangleAlert}
            tone="error"
            title="Can’t reach the server"
            detail="The REST API didn’t respond. Check that the server is running and RESTAPIEnabled=True."
          />
        ) : (
          <>
            <div className="hero-row">
              {/* Hero identity + gauge */}
              <div className="card card--pad">
                <div className="card__glow" />
                <div className="hero">
                  {m ? (
                    <Gauge value={m.serverfps} max={60} label="Server FPS" display={Math.round(m.serverfps)} unit="Server FPS" />
                  ) : (
                    <Skeleton width={190} height={190} radius="50%" />
                  )}
                  <div className="hero__meta">
                    {m ? (
                      <StatusPill tone={healthy ? "good" : "warn"} label={healthy ? "ONLINE · HEALTHY" : "ONLINE · STRAINED"} pulse />
                    ) : (
                      <Skeleton width={150} height={26} radius="var(--r-full)" />
                    )}
                    {info.data ? (
                      <>
                        <h2>{info.data.servername}</h2>
                        <div className="hero__sub">
                          {info.data.version} · world {info.data.worldguid.slice(0, 4)}…{info.data.worldguid.slice(-3)}
                        </div>
                      </>
                    ) : (
                      <div style={{ marginTop: 14, display: "flex", flexDirection: "column", gap: 8 }}>
                        <Skeleton width={220} height={22} />
                        <Skeleton width={140} height={13} />
                      </div>
                    )}
                    <div className="hero__tags">
                      {s ? (
                        <>
                          <span className="tag">Difficulty <b>{String(s.Difficulty || "None")}</b></span>
                          <span className="tag">PvP <b>{s.bIsPvP ? "On" : "Off"}</b></span>
                          <span className="tag">Slots <b>{s.ServerPlayerMaxNum ?? m?.maxplayernum ?? "—"}</b></span>
                        </>
                      ) : (
                        <>
                          <Skeleton width={110} height={28} radius="var(--r-xs)" />
                          <Skeleton width={70} height={28} radius="var(--r-xs)" />
                        </>
                      )}
                    </div>
                  </div>
                </div>
              </div>

              {/* Quick actions */}
              <div className="card card--pad">
                <div className="eyebrow qa-title">Quick Actions</div>
                <div className="qa-grid">
                  <button className="qa" onClick={() => navigate("console")}>
                    <span className="qa__ic"><Megaphone size={16} /></span>
                    <span className="qa__t"><b>Announce</b><small>Broadcast message</small></span>
                  </button>
                  <button
                    className="qa"
                    onClick={() =>
                      setConfirm({
                        title: "Save the world?",
                        body: "Forces the server to write a save checkpoint now.",
                        confirmText: "Save world",
                        onConfirm: async () => {
                          const r = await api.saveWorld();
                          r.ok ? toast.success("World saved", r.message) : toast.error("Save failed", r.message);
                        },
                      })
                    }
                  >
                    <span className="qa__ic"><Save size={16} /></span>
                    <span className="qa__t"><b>Save World</b><small>Force checkpoint</small></span>
                  </button>
                  <button className="qa" onClick={() => navigate("players")}>
                    <span className="qa__ic"><Users size={16} /></span>
                    <span className="qa__t"><b>Players</b><small>Manage roster</small></span>
                  </button>
                  <button className="qa qa--danger" onClick={() => navigate("console")}>
                    <span className="qa__ic"><Power size={16} /></span>
                    <span className="qa__t"><b>Shutdown</b><small>With countdown</small></span>
                  </button>
                </div>
              </div>
            </div>

            {/* Metric tiles */}
            <div className="tiles-4">
              <MetricTile
                icon={Users}
                label="Players"
                value={m ? m.currentplayernum : "—"}
                suffix={m ? ` / ${m.maxplayernum}` : undefined}
                points={hist.players}
                stroke="var(--accent)"
              />
              <MetricTile icon={Clock} label="Uptime" value={m ? formatUptime(m.uptime) : "—"} points={undefined} />
              <MetricTile
                icon={Timer}
                label="Frame Time"
                value={m ? formatMs(m.serverframetime) : "—"}
                unit="ms"
                points={hist.frame}
                stroke="var(--accent-2)"
              />
              <MetricTile
                icon={Zap}
                label="Server FPS"
                value={m ? Math.round(m.serverfps) : "—"}
                delta={m ? { text: healthy ? "stable" : "low", tone: healthy ? "good" : "bad" } : undefined}
                points={hist.fps}
                stroke="var(--accent)"
              />
            </div>

            <div className="section-head">
              <h2 className="row" style={{ gap: 8 }}>
                <Activity size={16} style={{ color: "var(--accent)" }} /> Live snapshot
              </h2>
            </div>
            <div className="card card--pad" style={{ color: "var(--dim)", fontSize: 13 }}>
              Metrics refresh automatically. Use the World Map for a live view of every player and Pal, or the Console to
              broadcast, save, or schedule a shutdown.
            </div>
          </>
        )}
      </div>
      <ConfirmDialog spec={confirm} onClose={() => setConfirm(null)} />
    </>
  );
}
