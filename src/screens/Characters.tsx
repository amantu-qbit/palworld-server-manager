import "./characters.css";
import { useMemo, useState } from "react";
import { TriangleAlert, Users } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { DataTable } from "../components/DataTable";
import type { Column } from "../components/DataTable";
import { Drawer } from "../components/Drawer";
import { EmptyState } from "../components/EmptyState";
import { Skeleton } from "../components/Skeleton";
import { Input } from "../components/Field";
import {
  useBridgeGuilds,
  useBridgePlayerDetail,
  useBridgePlayers,
  useBridgeReference,
} from "../hooks/bridge";
import type { ItemContainer, PlayerSummary } from "../types/bridge";

type SortKey = "nickname" | "level" | "pal_count";

/** "SheepBall" → "Sheep Ball"; "Pal_Egg" → "Pal Egg". */
const humanize = (s: string) =>
  s
    .replace(/_/g, " ")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .trim();

export function Characters() {
  const players = useBridgePlayers();
  const guilds = useBridgeGuilds();
  const [search, setSearch] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("level");
  const [sortDir, setSortDir] = useState<"asc" | "desc">("desc");
  const [selected, setSelected] = useState<PlayerSummary | null>(null);

  const guildName = useMemo(() => {
    const m = new Map<string, string>();
    for (const g of guilds.data ?? []) m.set(g.id, g.name);
    return m;
  }, [guilds.data]);

  const data = players.data;

  const rows = useMemo(() => {
    const list = data ?? [];
    const q = search.trim().toLowerCase();
    const filtered = q
      ? list.filter((p) => p.nickname.toLowerCase().includes(q) || p.uid.toLowerCase().includes(q))
      : list;
    const sorted = [...filtered].sort((a, b) => {
      const cmp =
        sortKey === "nickname"
          ? a.nickname.localeCompare(b.nickname)
          : (a[sortKey] as number) - (b[sortKey] as number);
      return sortDir === "asc" ? cmp : -cmp;
    });
    return sorted;
  }, [data, search, sortKey, sortDir]);

  const onSort = (key: string) => {
    if (key === sortKey) setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    else setSortKey(key as SortKey);
  };

  const columns: Column<PlayerSummary>[] = [
    {
      key: "nickname",
      header: "Character",
      sortable: true,
      render: (p) => (
        <div className="ch-name">
          <b>{p.nickname || "(unnamed)"}</b>
          <small>{p.uid}</small>
        </div>
      ),
    },
    {
      key: "level",
      header: "Level",
      align: "right",
      sortable: true,
      render: (p) => <span style={{ fontFamily: "var(--mono)" }}>{p.level}</span>,
    },
    {
      key: "pal_count",
      header: "Pals",
      align: "right",
      sortable: true,
      render: (p) => <span style={{ fontFamily: "var(--mono)" }}>{p.pal_count}</span>,
    },
    {
      key: "guild",
      header: "Guild",
      render: (p) => (
        <span style={{ color: "var(--dim)" }}>
          {(p.guild_id && guildName.get(p.guild_id)) || (p.guild_id ? "—" : "No guild")}
        </span>
      ),
    },
  ];

  return (
    <>
      <TopBar
        breadcrumb="Server+"
        title="Characters"
        onRefresh={() => players.refetch()}
        refreshing={players.isFetching}
      />
      <div className="page">
        {players.isError ? (
          <EmptyState
            icon={TriangleAlert}
            tone="error"
            title="Can’t reach the bridge"
            detail={(players.error as Error)?.message ?? "The bridge didn’t respond."}
          />
        ) : players.isLoading && !data ? (
          <div className="card card--pad col" style={{ gap: 12 }}>
            {Array.from({ length: 6 }).map((_, i) => (
              <Skeleton key={i} height={44} radius="var(--r-md)" />
            ))}
          </div>
        ) : data && data.length === 0 ? (
          <EmptyState
            icon={Users}
            title="No characters found"
            detail="The bridge decoded the save but found no player characters."
          />
        ) : (
          <>
            <div className="row wrap ch-toolbar">
              <div className="ch-search">
                <Input
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  placeholder="Search by name or UID…"
                />
              </div>
              <span className="chip chip--accent">
                <span className="chip__dot" />
                {data?.length ?? 0} characters
              </span>
            </div>
            {rows.length === 0 ? (
              <div className="ch-nomatch">No matches.</div>
            ) : (
              <DataTable<PlayerSummary>
                columns={columns}
                rows={rows}
                rowKey={(p) => p.uid}
                onRowClick={(p) => setSelected(p)}
                sortKey={sortKey}
                sortDir={sortDir}
                onSort={onSort}
              />
            )}
          </>
        )}
      </div>

      <Drawer
        open={!!selected}
        onClose={() => setSelected(null)}
        title={selected?.nickname || "(unnamed)"}
        subtitle={selected?.uid}
      >
        {selected && <CharacterDetail uid={selected.uid} summary={selected} guildName={guildName} />}
      </Drawer>
    </>
  );
}

function CharacterDetail({
  uid,
  summary,
  guildName,
}: {
  uid: string;
  summary: PlayerSummary;
  guildName: Map<string, string>;
}) {
  const detail = useBridgePlayerDetail(uid);
  const items = useBridgeReference("items");
  const itemName = (staticId: string) => items.data?.[staticId] ?? humanize(staticId);

  return (
    <>
      <div className="kv">
        <span className="kv__k">Level</span>
        <span className="kv__v">{summary.level}</span>
        <span className="kv__k">Pals</span>
        <span className="kv__v">{summary.pal_count}</span>
        <span className="kv__k">Guild</span>
        <span className="kv__v">
          {(summary.guild_id && guildName.get(summary.guild_id)) || (summary.guild_id ? "—" : "None")}
        </span>
        <span className="kv__k">UID</span>
        <span className="kv__v">{summary.uid}</span>
      </div>

      {detail.isLoading ? (
        <div className="col" style={{ gap: 8, marginTop: 14 }}>
          <Skeleton height={40} radius="var(--r-md)" />
          <Skeleton height={40} radius="var(--r-md)" />
          <Skeleton height={40} radius="var(--r-md)" />
        </div>
      ) : detail.isError ? (
        <p className="ch-detailerr">{(detail.error as Error)?.message ?? "Failed to load detail."}</p>
      ) : detail.data ? (
        <>
          <section className="ch-section">
            <h4>Pals · {detail.data.pals.length}</h4>
            {detail.data.pals.length === 0 ? (
              <p className="ch-empty">No pals in this character’s boxes.</p>
            ) : (
              <div className="ch-pals">
                {detail.data.pals.map((p) => (
                  <div key={p.instance_id} className="ch-pal">
                    <div className="ch-pal__main">
                      <b>{humanize(p.character_id) || "Pal"}</b>
                      {p.nickname && p.nickname !== p.character_id && <small>“{p.nickname}”</small>}
                    </div>
                    <div className="ch-pal__meta">
                      <span className="ch-pal__lv">Lv {p.level}</span>
                      <span className="ch-pal__iv" title="IV: HP / Attack / Defense">
                        IV {p.talent_hp}/{p.talent_shot}/{p.talent_defense}
                      </span>
                      {p.is_lucky && <span className="ch-badge ch-badge--lucky">Lucky</span>}
                      {p.is_boss && <span className="ch-badge ch-badge--boss">Alpha</span>}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </section>

          <section className="ch-section">
            <h4>Inventory</h4>
            {detail.data.inventory.every((c) => c.slots.every((s) => !s.static_id)) ? (
              <p className="ch-empty">Empty (or stored in a per-player save not on disk).</p>
            ) : (
              detail.data.inventory.map((c) => (
                <InventoryContainer key={c.id} container={c} itemName={itemName} />
              ))
            )}
          </section>
        </>
      ) : null}
    </>
  );
}

function InventoryContainer({
  container,
  itemName,
}: {
  container: ItemContainer;
  itemName: (staticId: string) => string;
}) {
  const filled = container.slots.filter((s) => s.static_id);
  if (filled.length === 0) return null;
  return (
    <div className="ch-invgroup">
      <div className="ch-invgroup__head">{humanize(container.container_type)}</div>
      {filled.map((s) => (
        <div key={s.slot_index} className="ch-item">
          <span className="ch-item__name">
            {itemName(s.static_id)}
            {s.dynamic_item?.egg_params ? " (egg)" : ""}
          </span>
          <span className="ch-item__count">×{s.count}</span>
        </div>
      ))}
    </div>
  );
}
