import "./worldmap.css";
import { useMemo, useState } from "react";
import { TriangleAlert } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { EmptyState } from "../components/EmptyState";
import { Skeleton } from "../components/Skeleton";
import { WorldMapView } from "../components/WorldMapView";
import { useGameData, usePlayers } from "../hooks/queries";
import { worldToGameCoords } from "../lib/mapProject";
import type { Actor } from "../types/api";

interface UnitDef {
  type: string;
  label: string;
  color: string;
}

const UNIT_TYPES: UnitDef[] = [
  { type: "Player", label: "Players", color: "var(--accent)" },
  { type: "WildPal", label: "Wild Pals", color: "#aab2c0" },
  { type: "BaseCampPal", label: "Base Pals", color: "var(--good)" },
  { type: "OtomoPal", label: "Otomo Pals", color: "var(--accent-2)" },
  { type: "NPC", label: "NPCs", color: "var(--warn)" },
];

export function WorldMap() {
  const gd = useGameData();
  const playersQ = usePlayers();
  const [hover, setHover] = useState<Actor | null>(null);
  const [visible, setVisible] = useState<Set<string>>(() => new Set(UNIT_TYPES.map((u) => u.type)));

  const snapshot = gd.data ?? null;
  // When /game-data is unavailable (e.g. the GameData API isn't enabled on the
  // server), fall back to plotting just players — /players carries their coords.
  const fallback = !snapshot;

  const actors: Actor[] = useMemo(() => {
    if (snapshot) return snapshot.ActorData;
    return (playersQ.data ?? []).map((p) => ({
      Type: "Character",
      InstanceID: p.playerId,
      UnitType: "Player",
      NickName: p.name,
      userid: p.userId,
      ip: p.ip,
      level: p.level,
      LocationX: p.location_x,
      LocationY: p.location_y,
      LocationZ: 0,
    }));
  }, [snapshot, playersQ.data]);

  const counts = useMemo(() => {
    const c: Record<string, number> = {};
    for (const a of actors) c[a.UnitType] = (c[a.UnitType] ?? 0) + 1;
    return c;
  }, [actors]);

  const toggle = (type: string) =>
    setVisible((prev) => {
      const next = new Set(prev);
      if (next.has(type)) next.delete(type);
      else next.add(type);
      return next;
    });

  const coords = hover ? worldToGameCoords(hover.LocationX, hover.LocationY) : null;

  const loading = fallback ? playersQ.isLoading && !playersQ.data : false;
  const errored = fallback && playersQ.isError && !playersQ.data;

  return (
    <>
      <TopBar
        breadcrumb="Overview"
        title="World Map"
        showLive
        onRefresh={() => {
          gd.refetch();
          playersQ.refetch();
        }}
        refreshing={gd.isFetching || playersQ.isFetching}
      />
      <div className="page">
        {errored ? (
          <EmptyState
            icon={TriangleAlert}
            tone="error"
            title="Couldn’t load the world map"
            detail={
              (playersQ.error instanceof Error && playersQ.error.message) ||
              "The server didn’t respond. Check that it’s running and the REST API is enabled."
            }
          />
        ) : loading ? (
          <div className="wm-grid">
            <div className="card">
              <Skeleton style={{ width: "100%", height: "auto", aspectRatio: "1 / 1", borderRadius: "var(--r-lg)" }} />
            </div>
            <div className="wm-side">
              <div className="card card--pad">
                <Skeleton width={90} height={12} />
                <Skeleton height={54} radius="var(--r-sm)" style={{ marginTop: 14 }} />
              </div>
            </div>
          </div>
        ) : (
          <div className="wm-grid">
            {/* Map */}
            <div className="card wm-map-card">
              <WorldMapView actors={actors} visible={visible} onHover={setHover} />
              <div className="wm-attrib">Map artwork © Pocketpair, Inc. · scroll to zoom, drag to pan</div>
            </div>

            <div className="wm-side">
              {fallback && (
                <div className="wm-hint">
                  <TriangleAlert size={15} />
                  <div>
                    <b>Showing players only.</b> The server’s GameData API is off, so Pals aren’t
                    available. Add <code>-enable-gamedata-api -collect-gamedata-interval=60</code> to your
                    server’s startup command to see every Pal.
                  </div>
                </div>
              )}

              {/* Snapshot (only when the full game-data snapshot is available) */}
              {snapshot && (
                <div className="card card--pad">
                  <div className="eyebrow" style={{ marginBottom: 12 }}>
                    Snapshot
                  </div>
                  <div className="kv">
                    <span className="kv__k">Time</span>
                    <span className="kv__v">{snapshot.Time}</span>
                    <span className="kv__k">FPS</span>
                    <span className="kv__v">{Math.round(snapshot.FPS)}</span>
                    <span className="kv__k">Average FPS</span>
                    <span className="kv__v">{Math.round(snapshot.AverageFPS)}</span>
                    <span className="kv__k">Actors</span>
                    <span className="kv__v">{snapshot.ActorData.length}</span>
                  </div>
                </div>
              )}

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
                {hover && coords ? (
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
                      {coords.x}, {coords.y}
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
                    <p className="wm-legend__hint">Hover a marker to inspect it. Coordinates match the in-game map.</p>
                  </div>
                )}
              </div>
            </div>
          </div>
        )}
      </div>
    </>
  );
}
