import "./worldmap.css";
import { useMemo, useState } from "react";
import { TriangleAlert } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { EmptyState } from "../components/EmptyState";
import { Skeleton } from "../components/Skeleton";
import { Radar } from "../components/Radar";
import { useGameData } from "../hooks/queries";
import { formatCoord } from "../lib/format";
import type { Actor } from "../types/api";

interface UnitDef {
  type: string;
  label: string;
  color: string;
}

const UNIT_TYPES: UnitDef[] = [
  { type: "Player", label: "Players", color: "var(--accent)" },
  { type: "WildPal", label: "Wild Pals", color: "var(--faint)" },
  { type: "BaseCampPal", label: "Base Pals", color: "var(--good)" },
  { type: "OtomoPal", label: "Otomo Pals", color: "var(--accent-2)" },
  { type: "NPC", label: "NPCs", color: "var(--mute)" },
];

export function WorldMap() {
  const q = useGameData();
  const data = q.data;
  const [hover, setHover] = useState<Actor | null>(null);
  const [visible, setVisible] = useState<Set<string>>(() => new Set(UNIT_TYPES.map((u) => u.type)));

  const counts = useMemo(() => {
    const c: Record<string, number> = {};
    for (const a of data?.ActorData ?? []) c[a.UnitType] = (c[a.UnitType] ?? 0) + 1;
    return c;
  }, [data]);

  const toggle = (type: string) =>
    setVisible((prev) => {
      const next = new Set(prev);
      if (next.has(type)) next.delete(type);
      else next.add(type);
      return next;
    });

  return (
    <>
      <TopBar breadcrumb="Overview" title="World Map" showLive onRefresh={() => q.refetch()} refreshing={q.isFetching} />
      <div className="page">
        {q.isError ? (
          <EmptyState
            icon={TriangleAlert}
            tone="error"
            title="Can’t reach the server"
            detail="The REST API didn’t respond. Check the server is running and RESTAPIEnabled=True."
          />
        ) : q.isLoading && !data ? (
          <div className="wm-grid">
            <div className="card card--pad">
              <Skeleton radius="var(--r-md)" style={{ width: "100%", height: "auto", aspectRatio: "1 / 1" }} />
            </div>
            <div className="wm-side">
              <div className="card card--pad">
                <Skeleton width={90} height={12} />
                <Skeleton height={54} radius="var(--r-sm)" style={{ marginTop: 14 }} />
              </div>
              <div className="card card--pad">
                <Skeleton width={70} height={12} />
                <Skeleton height={172} radius="var(--r-sm)" style={{ marginTop: 14 }} />
              </div>
            </div>
          </div>
        ) : data ? (
          <div className="wm-grid">
            {/* Radar */}
            <div className="card card--pad wm-radar-card">
              <div className="card__glow" />
              <Radar actors={data.ActorData} visible={visible} onHover={setHover} />
            </div>

            <div className="wm-side">
              {/* Snapshot */}
              <div className="card card--pad">
                <div className="eyebrow" style={{ marginBottom: 12 }}>
                  Snapshot
                </div>
                <div className="kv">
                  <span className="kv__k">Time</span>
                  <span className="kv__v">{data.Time}</span>
                  <span className="kv__k">FPS</span>
                  <span className="kv__v">{Math.round(data.FPS)}</span>
                  <span className="kv__k">Average FPS</span>
                  <span className="kv__v">{Math.round(data.AverageFPS)}</span>
                </div>
              </div>

              {/* Filters */}
              <div className="card card--pad">
                <div className="eyebrow" style={{ marginBottom: 12 }}>
                  Filters
                </div>
                <div className="wm-filters">
                  {UNIT_TYPES.map((u) => {
                    const on = visible.has(u.type);
                    return (
                      <button
                        key={u.type}
                        className={`wm-filter${on ? "" : " wm-filter--off"}`}
                        onClick={() => toggle(u.type)}
                        aria-pressed={on}
                      >
                        <span className="wm-dot" style={{ background: u.color }} />
                        <span className="wm-filter__label">{u.label}</span>
                        <span className="wm-filter__count">{counts[u.type] ?? 0}</span>
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Hovered actor / legend */}
              <div className="card card--pad">
                <div className="eyebrow" style={{ marginBottom: 12 }}>
                  {hover ? "Selected" : "Legend"}
                </div>
                {hover ? (
                  <div className="kv">
                    <span className="kv__k">Name</span>
                    <span className="kv__v">{hover.NickName || hover.Class || hover.UnitType}</span>
                    <span className="kv__k">Type</span>
                    <span className="kv__v">{hover.UnitType}</span>
                    {hover.level != null && (
                      <>
                        <span className="kv__k">Level</span>
                        <span className="kv__v">{hover.level}</span>
                      </>
                    )}
                    {hover.HP != null && hover.MaxHP != null && (
                      <>
                        <span className="kv__k">HP</span>
                        <span className="kv__v">
                          {hover.HP} / {hover.MaxHP}
                        </span>
                      </>
                    )}
                    {hover.GuildName && (
                      <>
                        <span className="kv__k">Guild</span>
                        <span className="kv__v">{hover.GuildName}</span>
                      </>
                    )}
                    <span className="kv__k">Coords</span>
                    <span className="kv__v">
                      {formatCoord(hover.LocationX)}, {formatCoord(hover.LocationY)}
                    </span>
                  </div>
                ) : (
                  <div className="wm-legend">
                    {UNIT_TYPES.map((u) => (
                      <div className="wm-legend__row" key={u.type}>
                        <span className="wm-dot" style={{ background: u.color }} />
                        <span>{u.label}</span>
                      </div>
                    ))}
                    <p className="wm-legend__hint">Hover a point on the radar to inspect it. North is up.</p>
                  </div>
                )}
              </div>
            </div>
          </div>
        ) : null}
      </div>
    </>
  );
}
