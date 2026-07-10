import { Pause, Play, RefreshCw } from "lucide-react";
import { POLL_OPTIONS, usePrefs } from "../store/prefs";

interface Props {
  breadcrumb: string;
  title: string;
  onRefresh?: () => void;
  refreshing?: boolean;
  /** Show the live-refresh controls (dashboard-style screens). */
  showLive?: boolean;
}

export function TopBar({ breadcrumb, title, onRefresh, refreshing, showLive = true }: Props) {
  const { pollInterval, setPollInterval, paused, setPaused } = usePrefs();

  return (
    <header className="topbar">
      <div className="topbar__title">
        <div className="topbar__crumb">{breadcrumb}</div>
        <h1>{title}</h1>
      </div>

      {showLive && (
        <div className="pill topbar__live">
          <span className={paused ? "" : "livedot"} style={paused ? { opacity: 0.4 } : undefined} />
          <span>{paused ? "Paused" : "Live"}</span>
          <select
            className="topbar__interval"
            value={pollInterval}
            onChange={(e) => setPollInterval(Number(e.target.value))}
            aria-label="Refresh interval"
            disabled={paused}
          >
            {POLL_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
          <button
            className="topbar__pause"
            onClick={() => setPaused(!paused)}
            aria-label={paused ? "Resume auto-refresh" : "Pause auto-refresh"}
            title={paused ? "Resume" : "Pause"}
          >
            {paused ? <Play size={13} /> : <Pause size={13} />}
          </button>
        </div>
      )}

      {onRefresh && (
        <button className="icobtn" onClick={onRefresh} aria-label="Refresh now" title="Refresh now">
          <RefreshCw size={16} className={refreshing ? "spin" : undefined} />
        </button>
      )}
    </header>
  );
}
