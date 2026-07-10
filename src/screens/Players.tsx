import "./players.css";
import { useMemo, useState } from "react";
import { TriangleAlert, Users } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { DataTable } from "../components/DataTable";
import type { Column } from "../components/DataTable";
import { Drawer } from "../components/Drawer";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { ConfirmSpec } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { Skeleton } from "../components/Skeleton";
import { Button } from "../components/Button";
import { Input } from "../components/Field";
import { usePlayers } from "../hooks/queries";
import { useToast } from "../hooks/useToast";
import { api } from "../api";
import type { Player } from "../types/api";

type SortKey = "name" | "level" | "ping" | "building_count";

function pingColor(ping: number): string {
  if (ping <= 60) return "var(--good)";
  if (ping <= 100) return "var(--warn)";
  return "var(--bad)";
}

export function Players() {
  const players = usePlayers();
  const toast = useToast();
  const [search, setSearch] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("level");
  const [sortDir, setSortDir] = useState<"asc" | "desc">("desc");
  const [selected, setSelected] = useState<Player | null>(null);
  const [confirm, setConfirm] = useState<ConfirmSpec | null>(null);

  const data = players.data;

  const rows = useMemo(() => {
    const list = data ?? [];
    const q = search.trim().toLowerCase();
    const filtered = q
      ? list.filter(
          (p) =>
            p.name.toLowerCase().includes(q) ||
            p.accountName.toLowerCase().includes(q) ||
            p.userId.toLowerCase().includes(q),
        )
      : list;
    const sorted = [...filtered].sort((a, b) => {
      const cmp =
        sortKey === "name"
          ? a.name.localeCompare(b.name)
          : (a[sortKey] as number) - (b[sortKey] as number);
      return sortDir === "asc" ? cmp : -cmp;
    });
    return sorted;
  }, [data, search, sortKey, sortDir]);

  const onSort = (key: string) => {
    if (key === sortKey) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key as SortKey);
    }
  };

  const columns: Column<Player>[] = [
    {
      key: "name",
      header: "Player",
      sortable: true,
      render: (p) => (
        <div className="pl-name">
          <b>{p.name}</b>
          <small>{p.accountName}</small>
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
      key: "ping",
      header: "Ping",
      align: "right",
      sortable: true,
      render: (p) => (
        <span style={{ fontFamily: "var(--mono)", color: pingColor(p.ping) }}>{p.ping}ms</span>
      ),
    },
    {
      key: "building_count",
      header: "Buildings",
      align: "right",
      sortable: true,
      render: (p) => <span style={{ fontFamily: "var(--mono)" }}>{p.building_count}</span>,
    },
    {
      key: "userId",
      header: "User ID",
      render: (p) => (
        <span
          style={{
            fontFamily: "var(--mono)",
            color: "var(--dim)",
            display: "inline-block",
            maxWidth: 150,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            verticalAlign: "middle",
          }}
        >
          {p.userId}
        </span>
      ),
    },
  ];

  const kick = (p: Player) =>
    setConfirm({
      title: "Kick " + p.name + "?",
      body: "They can rejoin immediately.",
      confirmText: "Kick",
      onConfirm: async () => {
        const r = await api.kick(p.userId, "Kicked by admin");
        r.ok ? toast.success("Player kicked", r.message) : toast.error("Kick failed", r.message);
        players.refetch();
        setSelected(null);
      },
    });

  const ban = (p: Player) =>
    setConfirm({
      title: "Ban " + p.name + "?",
      body: "They will be disconnected and blocked.",
      confirmText: "Ban player",
      danger: true,
      requireText: p.name,
      onConfirm: async () => {
        const r = await api.ban(p.userId, "Banned by admin");
        r.ok ? toast.success("Player banned", r.message) : toast.error("Ban failed", r.message);
        players.refetch();
        setSelected(null);
      },
    });

  return (
    <>
      <TopBar
        breadcrumb="Overview"
        title="Players"
        showLive
        onRefresh={() => players.refetch()}
        refreshing={players.isFetching}
      />
      <div className="page">
        {players.isError ? (
          <EmptyState
            icon={TriangleAlert}
            tone="error"
            title="Can’t reach the server"
            detail="The REST API didn’t respond. Check the server is running and RESTAPIEnabled=True."
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
            title="No players online"
            detail="Players appear here when they join."
          />
        ) : (
          <>
            <div className="row wrap pl-toolbar">
              <div className="pl-search">
                <Input
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  placeholder="Search by name, account, or user ID…"
                />
              </div>
              <span className="chip chip--accent">
                <span className="chip__dot" />
                {data?.length ?? 0} online
              </span>
            </div>
            {rows.length === 0 ? (
              <div className="pl-nomatch">No matches.</div>
            ) : (
              <DataTable<Player>
                columns={columns}
                rows={rows}
                rowKey={(p) => p.playerId}
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
        title={selected?.name ?? ""}
        subtitle={selected?.userId}
      >
        {selected && (
          <>
            <div className="kv">
              <span className="kv__k">Account</span>
              <span className="kv__v">{selected.accountName}</span>
              <span className="kv__k">Player ID</span>
              <span className="kv__v">{selected.playerId}</span>
              <span className="kv__k">User ID</span>
              <span className="kv__v">{selected.userId}</span>
              <span className="kv__k">IP</span>
              <span className="kv__v">{selected.ip}</span>
              <span className="kv__k">Ping</span>
              <span className="kv__v">{selected.ping} ms</span>
              <span className="kv__k">Level</span>
              <span className="kv__v">{selected.level}</span>
              <span className="kv__k">Buildings</span>
              <span className="kv__v">{selected.building_count}</span>
              <span className="kv__k">X</span>
              <span className="kv__v">{selected.location_x}</span>
              <span className="kv__k">Y</span>
              <span className="kv__v">{selected.location_y}</span>
            </div>
            <div className="row pl-actions">
              <Button variant="ghost" onClick={() => kick(selected)}>
                Kick
              </Button>
              <Button variant="danger" onClick={() => ban(selected)}>
                Ban
              </Button>
            </div>
          </>
        )}
      </Drawer>

      <ConfirmDialog spec={confirm} onClose={() => setConfirm(null)} />
    </>
  );
}
