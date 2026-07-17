import { useMemo, useState } from "react";
import type { ReactNode } from "react";
import { Activity, Timer, Trash2, Users, Zap } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { TimeSeriesChart } from "../components/TimeSeriesChart";
import type { ChartPoint } from "../components/TimeSeriesChart";
import { EmptyState } from "../components/EmptyState";
import { clearHistory, useMetricsHistory } from "../store/metricsHistory";
import "./trends.css";

const WINDOWS = [
  { id: "1h", label: "1h", ms: 3_600_000 },
  { id: "6h", label: "6h", ms: 21_600_000 },
  { id: "24h", label: "24h", ms: 86_400_000 },
] as const;

type WindowId = (typeof WINDOWS)[number]["id"];

export function Trends() {
  const history = useMetricsHistory();
  const [win, setWin] = useState<WindowId>("1h");

  const windowMs = WINDOWS.find((w) => w.id === win)!.ms;
  // Recompute the cut-off on each render so the window tracks wall-clock time.
  const cutoff = Date.now() - windowMs;
  const samples = useMemo(() => history.filter((s) => s.t >= cutoff), [history, cutoff]);

  const series = (key: "fps" | "players" | "frame"): ChartPoint[] =>
    samples.map((s) => ({ t: s.t, v: s[key] }));

  const maxPlayers = samples.reduce((m, s) => Math.max(m, s.max), 0);
  const restarts = useMemo(() => {
    let n = 0;
    for (let i = 1; i < samples.length; i++) {
      if (samples[i].uptime < samples[i - 1].uptime) n++;
    }
    return n;
  }, [samples]);

  const enough = samples.length >= 2;

  return (
    <>
      <TopBar breadcrumb="Overview" title="Trends" showLive={false} />
      <div className="page">
        <div className="tr-head">
          <p className="tr-note">
            <Activity size={13} /> Collected while the manager is open and connected — breaks in
            a line mark times it was closed, paused, or offline.
          </p>
          <div className="tr-controls">
            <div className="tr-windows" role="tablist">
              {WINDOWS.map((w) => (
                <button
                  key={w.id}
                  role="tab"
                  aria-selected={win === w.id}
                  className={`tr-win${win === w.id ? " is-on" : ""}`}
                  onClick={() => setWin(w.id)}
                >
                  {w.label}
                </button>
              ))}
            </div>
            <button className="tr-clear" onClick={() => clearHistory()} title="Clear stored history">
              <Trash2 size={13} /> Clear
            </button>
          </div>
        </div>

        {!enough ? (
          <EmptyState
            icon={Activity}
            title="Building history…"
            detail="Trend graphs appear once a couple of minutes of samples have been collected. Keep the app open and connected — history is kept per server."
          />
        ) : (
          <div className="tr-charts">
            <ChartCard
              icon={Zap}
              title="Server FPS"
              hint={restarts > 0 ? `${restarts} restart${restarts === 1 ? "" : "s"}` : undefined}
            >
              <TimeSeriesChart points={series("fps")} stroke="var(--accent)" yMin={0} yMax={62} unit=" fps" />
            </ChartCard>

            <ChartCard icon={Users} title="Players online">
              <TimeSeriesChart
                points={series("players")}
                stroke="var(--accent-2)"
                yMin={0}
                yMax={Math.max(4, maxPlayers)}
              />
            </ChartCard>

            <ChartCard icon={Timer} title="Frame time">
              <TimeSeriesChart
                points={series("frame")}
                stroke="#e0a15a"
                unit=" ms"
                format={(v) => v.toFixed(1)}
              />
            </ChartCard>
          </div>
        )}
      </div>
    </>
  );
}

function ChartCard({
  icon: Icon,
  title,
  hint,
  children,
}: {
  icon: LucideIcon;
  title: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div className="card card--pad tr-card">
      <div className="between row">
        <div className="eyebrow tr-card__t">
          <Icon size={13} /> {title}
        </div>
        {hint && <span className="tr-card__hint">{hint}</span>}
      </div>
      {children}
    </div>
  );
}
