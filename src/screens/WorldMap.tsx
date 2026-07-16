import "./worldmap.css";
import { useMemo } from "react";
import { TriangleAlert } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { EmptyState } from "../components/EmptyState";
import { Skeleton } from "../components/Skeleton";
import { WorldMapView } from "../components/WorldMapView";
import { useGameData, usePlayers } from "../hooks/queries";
import { useBridge, useBridgeGuilds } from "../hooks/bridge";
import type { Actor } from "../types/api";

export function WorldMap() {
  const gd = useGameData();
  const playersQ = usePlayers();
  const bridge = useBridge();
  // Real base camps from the save (when the bridge is available) → accurate
  // build-area circles on the map, instead of the base-pal approximation.
  const guildsQ = useBridgeGuilds(bridge.available);
  const bases = useMemo(
    () =>
      (guildsQ.data ?? []).flatMap((g) =>
        g.bases
          .filter((b) => b.position && b.area_range > 0)
          .map((b) => ({
            x: (b.position as [number, number, number])[0],
            y: (b.position as [number, number, number])[1],
            area_range: b.area_range,
            guild: g.name?.trim() || "Guild",
          })),
      ),
    [guildsQ.data],
  );

  const snapshot = gd.data ?? null;
  // When /game-data is unavailable (GameData API off), fall back to plotting the
  // connected players from /players.
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

  // Keys of currently-connected players (userId + names), used to filter out the
  // pawns of players who left the server but still linger in the game-data snapshot.
  const onlineKeys = useMemo(() => {
    const s = new Set<string>();
    for (const p of playersQ.data ?? []) {
      if (p.userId) s.add(p.userId);
      if (p.name) s.add(p.name.toLowerCase());
      if (p.accountName) s.add(p.accountName.toLowerCase());
    }
    return s;
  }, [playersQ.data]);

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
      <div className="page page--map">
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
          <div className="wm-canvas">
            <Skeleton style={{ width: "100%", height: "100%", borderRadius: "var(--r-lg)" }} />
          </div>
        ) : (
          <WorldMapView actors={actors} onlineKeys={onlineKeys} fallback={fallback} bases={bases} />
        )}
      </div>
    </>
  );
}
