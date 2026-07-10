import { useEffect, useState } from "react";
import { ArrowUpRight, Clock, Megaphone, Power, Save, Timer, TriangleAlert, Users, Zap } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { Gauge } from "../components/Gauge";
import { MetricTile } from "../components/MetricTile";
import { StatusPill } from "../components/StatusPill";
import { Sparkline } from "../components/Sparkline";
import { Skeleton } from "../components/Skeleton";
import { EmptyState } from "../components/EmptyState";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { ConfirmSpec } from "../components/ConfirmDialog";
import { useInfo, useMetrics, usePlayers, useSettings } from "../hooks/queries";
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
  const players = usePlayers();
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
  const roster = [...(players.data ?? [])].sort((a, b) => b.level - a.level).slice(0, 5);
  const capacity = m ? Math.round((m.currentplayernum / m.maxplayernum) * 100) : 0;

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
                      <StatusPill tone={healthy ? "good" : "warn"} label={healthy ? "Online" : "Strained"} pulse />
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

            {/* Metric cluster — one instrument panel, hairline-divided */}
            <div className="card metric-cluster">
              <MetricTile
                flat
                icon={Users}
                label="Players"
                value={m ? m.currentplayernum : "—"}
                suffix={m ? ` / ${m.maxplayernum}` : undefined}
                points={hist.players}
                stroke="var(--accent)"
              />
              <MetricTile flat icon={Clock} label="Uptime" value={m ? formatUptime(m.uptime) : "—"} />
              <MetricTile
                flat
                icon={Timer}
                label="Frame Time"
                value={m ? formatMs(m.serverframetime) : "—"}
                unit="ms"
                points={hist.frame}
                stroke="var(--accent-2)"
              />
              <MetricTile
                flat
                icon={Zap}
                label="Server FPS"
                value={m ? Math.round(m.serverfps) : "—"}
                delta={m ? { text: healthy ? "stable" : "low", tone: healthy ? "good" : "bad" } : undefined}
                points={hist.fps}
                stroke="var(--accent)"
              />
            </div>

            <div className="dash-lower">
              {/* Population */}
              <div className="card card--pad dash-pop">
                <div className="between row">
                  <div className="eyebrow">Population</div>
                  <span className="mono dash-pop__cap">{capacity}% full</span>
                </div>
                <div className="dash-pop__figure mono">
                  {m ? m.currentplayernum : "—"}
                  <small>/ {m?.maxplayernum ?? "—"} online</small>
                </div>
                <div className="dash-pop__chart">
                  <Sparkline points={hist.players.length > 1 ? hist.players : [0, 0]} height={72} stroke="var(--accent)" />
                </div>
              </div>

              {/* Roster preview */}
              <div className="card card--pad dash-roster">
                <div className="between row" style={{ marginBottom: 4 }}>
                  <div className="eyebrow">Top players</div>
                  <button className="dash-link" onClick={() => navigate("players")}>
                    All players <ArrowUpRight size={13} />
                  </button>
                </div>
                {roster.length === 0 ? (
                  <div className="dash-roster__empty">No players online.</div>
                ) : (
                  <ul className="roster">
                    {roster.map((p, i) => (
                      <li className="roster__row" key={p.playerId}>
                        <span className="roster__rank mono">{i + 1}</span>
                        <span className="roster__name">{p.name}</span>
                        <span className="roster__lvl mono">Lv {p.level}</span>
                        <span className="roster__ping mono">{p.ping}ms</span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </div>
          </>
        )}
      </div>
      <ConfirmDialog spec={confirm} onClose={() => setConfirm(null)} />
    </>
  );
}
