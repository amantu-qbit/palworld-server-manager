import "./characters.css";
import { useEffect, useMemo, useState } from "react";
import { Cpu, PackageOpen, PawPrint, Search, TriangleAlert, Users, X } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { EmptyState } from "../components/EmptyState";
import { Skeleton } from "../components/Skeleton";
import { PalIcon } from "../components/PalIcon";
import { ItemIcon } from "../components/ItemIcon";
import {
  useBridgeGuilds,
  useBridgePlayerDetail,
  useBridgePlayers,
  useBridgeReference,
} from "../hooks/bridge";
import { TechTree } from "../components/TechTree";
import { elementColor, isRare, palInfo } from "../lib/palDex";
import { humanize, statusLabel, workLabel } from "../lib/palLabels";
import type { ItemContainer, Pal, PlayerDetail, PlayerSummary } from "../types/bridge";

const genderSymbol = (g: string) => {
  const l = g.toLowerCase();
  return l.includes("female") ? "♀" : l.includes("male") ? "♂" : "";
};
const ivColor = (v: number) => (v >= 90 ? "#3ad19a" : v >= 60 ? "#e6b450" : "#7c8494");

export function Characters() {
  const players = useBridgePlayers();
  const guilds = useBridgeGuilds();
  const [search, setSearch] = useState("");
  const [selectedUid, setSelectedUid] = useState<string | null>(null);

  const guildName = useMemo(() => {
    const m = new Map<string, string>();
    for (const g of guilds.data ?? []) m.set(g.id, g.name);
    return m;
  }, [guilds.data]);

  const data = players.data;
  const list = useMemo(() => {
    const q = search.trim().toLowerCase();
    const rows = (data ?? []).filter(
      (p) => !q || p.nickname.toLowerCase().includes(q) || p.uid.toLowerCase().includes(q),
    );
    return [...rows].sort((a, b) => b.level - a.level || b.pal_count - a.pal_count);
  }, [data, search]);

  useEffect(() => {
    if (list.length && (!selectedUid || !list.some((p) => p.uid === selectedUid))) {
      setSelectedUid(list[0].uid);
    }
  }, [list, selectedUid]);

  const selected = list.find((p) => p.uid === selectedUid) ?? null;

  return (
    <>
      <TopBar
        breadcrumb="Server+"
        title="Characters"
        onRefresh={() => players.refetch()}
        refreshing={players.isFetching}
      />
      <div className="page ch-page">
        {players.isError ? (
          <EmptyState
            icon={TriangleAlert}
            tone="error"
            title="Can’t reach the bridge"
            detail={(players.error as Error)?.message ?? "The bridge didn’t respond."}
          />
        ) : players.isLoading && !data ? (
          <div className="card card--pad col" style={{ gap: 12 }}>
            {Array.from({ length: 5 }).map((_, i) => (
              <Skeleton key={i} height={48} radius="var(--r-md)" />
            ))}
          </div>
        ) : data && data.length === 0 ? (
          <EmptyState icon={Users} title="No characters found" detail="The save has no player characters." />
        ) : (
          <div className="ch">
            <aside className="ch-list">
              <div className="ch-search">
                <Search size={14} />
                <input
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  placeholder="Search characters…"
                />
              </div>
              <div className="ch-players">
                {list.map((p) => (
                  <button
                    key={p.uid}
                    className={`ch-prow${p.uid === selectedUid ? " is-active" : ""}`}
                    onClick={() => setSelectedUid(p.uid)}
                  >
                    <span className="ch-prow__name">{p.nickname || "(unnamed)"}</span>
                    <span className="ch-prow__meta">
                      Lv {p.level} · {p.pal_count} pals
                    </span>
                  </button>
                ))}
                {list.length === 0 && <div className="ch-nomatch">No matches.</div>}
              </div>
            </aside>

            <main className="ch-detail">
              {selected ? (
                <PlayerPanel key={selected.uid} summary={selected} guildName={guildName} />
              ) : (
                <div className="ch-detail__empty">
                  <PawPrint size={26} />
                  <p>Select a character to view their Pals, stats, and inventory.</p>
                </div>
              )}
            </main>
          </div>
        )}
      </div>
    </>
  );
}

type Tab = "pals" | "character" | "inventory";

function PlayerPanel({
  summary,
  guildName,
}: {
  summary: PlayerSummary;
  guildName: Map<string, string>;
}) {
  const detail = useBridgePlayerDetail(summary.uid);
  const items = useBridgeReference("items");
  const active = useBridgeReference("active_skills");
  const passive = useBridgeReference("passive_skills");
  const [tab, setTab] = useState<Tab>("pals");
  const [selectedPal, setSelectedPal] = useState<Pal | null>(null);

  const d = detail.data;
  const itemName = (id: string) => items.data?.[id] ?? humanize(id);
  const skillName = (code: string) => active.data?.[code] ?? humanize(code);
  const passiveName = (code: string) => passive.data?.[code] ?? humanize(code);
  const guild = (summary.guild_id && guildName.get(summary.guild_id)) || "No guild";

  const groups = useMemo(() => groupPals(d), [d]);

  return (
    <>
      <header className="ch-head">
        <h2>{summary.nickname || "(unnamed)"}</h2>
        <div className="ch-head__meta">
          <span className="ch-chip">Lv {summary.level}</span>
          <span className="ch-chip">{summary.pal_count} pals</span>
          <span className="ch-chip ch-chip--dim">{guild}</span>
        </div>
      </header>

      <div className="ch-tabs">
        <button className={tab === "pals" ? "is-on" : ""} onClick={() => setTab("pals")}>
          <PawPrint size={13} /> Pals {d ? `(${d.pals.length})` : ""}
        </button>
        <button className={tab === "character" ? "is-on" : ""} onClick={() => setTab("character")}>
          <Cpu size={13} /> Character
        </button>
        <button className={tab === "inventory" ? "is-on" : ""} onClick={() => setTab("inventory")}>
          <PackageOpen size={13} /> Inventory
        </button>
      </div>

      {detail.isLoading ? (
        <div className="ch-palgrid">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} height={150} radius="var(--r-md)" />
          ))}
        </div>
      ) : detail.isError ? (
        <p className="ch-err">{(detail.error as Error)?.message ?? "Failed to load."}</p>
      ) : d ? (
        <>
          {tab === "pals" &&
            (d.pals.length ? (
              groups.map((g) => (
                <section key={g.label} className="ch-palsection">
                  <div className="ch-palsection__head">
                    {g.label} <span>{g.pals.length}</span>
                  </div>
                  <div className="ch-palgrid">
                    {g.pals.map((pal) => (
                      <PalCard key={pal.instance_id} pal={pal} onClick={() => setSelectedPal(pal)} />
                    ))}
                  </div>
                </section>
              ))
            ) : (
              <p className="ch-empty">No Pals in this character’s boxes.</p>
            ))}

          {tab === "character" && <CharacterTab detail={d} />}

          {tab === "inventory" && <InventoryView inventory={d.inventory} itemName={itemName} />}
        </>
      ) : null}

      {selectedPal && (
        <PalDetailModal
          pal={selectedPal}
          onClose={() => setSelectedPal(null)}
          skillName={skillName}
          passiveName={passiveName}
        />
      )}
    </>
  );
}

interface PalGroup {
  label: string;
  pals: Pal[];
}
function groupPals(d: PlayerDetail | undefined): PalGroup[] {
  if (!d) return [];
  const party: Pal[] = [];
  const box: Pal[] = [];
  const base: Pal[] = [];
  for (const p of d.pals) {
    if (p.storage_id && p.storage_id === d.party_container) party.push(p);
    else if (p.storage_id && p.storage_id === d.pal_box_container) box.push(p);
    else base.push(p);
  }
  return [
    { label: "Party", pals: party },
    { label: "Pal Box", pals: box },
    { label: "Base & Expeditions", pals: base },
  ].filter((g) => g.pals.length > 0);
}

function PalCard({ pal, onClick }: { pal: Pal; onClick: () => void }) {
  const info = palInfo(pal.character_id);
  const rare = isRare(info.rarity);
  const accent = info.elements.length ? elementColor(info.elements[0]) : "#5a6070";
  const g = genderSymbol(pal.gender);

  return (
    <button className={`palcard${rare ? " palcard--rare" : ""}`} onClick={onClick}>
      <div className="palcard__iconwrap" style={{ ["--accent" as string]: accent }}>
        <PalIcon cell={info.cell} size={54} />
        <div className="palcard__badges">
          {pal.is_boss && <span className="pbadge pbadge--alpha">ALPHA</span>}
          {pal.is_lucky && <span className="pbadge pbadge--lucky">LUCKY</span>}
        </div>
      </div>
      <div className="palcard__name" title={info.name}>
        {info.name}
      </div>
      {pal.nickname && pal.nickname !== info.name ? (
        <div className="palcard__nick">“{pal.nickname}”</div>
      ) : (
        <div className="palcard__nick palcard__nick--ph" />
      )}
      <div className="palcard__meta">
        <span className="palcard__lv">Lv {pal.level}</span>
        {g && <span className="palcard__g">{g}</span>}
        {pal.rank > 0 && <span className="palcard__rank">★{pal.rank}</span>}
      </div>
      <div className="palcard__elems">
        {info.elements.map((el) => (
          <span key={el} className="elem" style={{ color: elementColor(el), background: `${elementColor(el)}22` }}>
            {el}
          </span>
        ))}
      </div>
      <div className="palcard__ivs">
        <IvBar label="HP" v={pal.talent_hp} />
        <IvBar label="ATK" v={pal.talent_shot} />
        <IvBar label="DEF" v={pal.talent_defense} />
      </div>
    </button>
  );
}

function IvBar({ label, v }: { label: string; v: number }) {
  const pct = Math.min(100, Math.max(0, v));
  return (
    <div className="iv" title={`${label} IV: ${v}`}>
      <span className="iv__l">{label}</span>
      <span className="iv__bar">
        <i style={{ width: `${pct}%`, background: ivColor(pct) }} />
      </span>
      <span className="iv__v">{v}</span>
    </div>
  );
}

function CharacterTab({ detail }: { detail: PlayerDetail }) {
  const stats = { ...detail.status_points, ...detail.ext_status_points };

  return (
    <div className="ch-char">
      <div className="ch-statgrid">
        <Stat label="Level" value={detail.level} />
        <Stat label="EXP" value={detail.exp.toLocaleString()} />
        {Object.entries(stats).map(([k, v]) => (
          <Stat key={k} label={statusLabel(k)} value={`+${v}`} />
        ))}
      </div>

      <TechTree
        unlocked={detail.technologies}
        techPoints={detail.technology_points}
        ancientPoints={detail.boss_technology_points}
      />
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="ch-stat">
      <span className="ch-stat__v">{value}</span>
      <span className="ch-stat__l">{label}</span>
    </div>
  );
}

function PalDetailModal({
  pal,
  onClose,
  skillName,
  passiveName,
}: {
  pal: Pal;
  onClose: () => void;
  skillName: (c: string) => string;
  passiveName: (c: string) => string;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const info = palInfo(pal.character_id);
  const accent = info.elements.length ? elementColor(info.elements[0]) : "#5a6070";
  const souls = [
    ["HP", pal.rank_hp],
    ["Attack", pal.rank_attack],
    ["Defense", pal.rank_defense],
    ["Craft", pal.rank_craftspeed],
  ] as const;
  const work = Object.entries(pal.work_suitability).filter(([, r]) => r > 0);

  return (
    <div className="ch-modal" onClick={onClose}>
      <div className="ch-modalcard" onClick={(e) => e.stopPropagation()}>
        <button className="ch-modalclose" onClick={onClose} aria-label="Close">
          <X size={16} />
        </button>
        <div className="ch-modalhead">
          <div className="palcard__iconwrap ch-modalicon" style={{ ["--accent" as string]: accent }}>
            <PalIcon cell={info.cell} size={64} />
          </div>
          <div className="ch-modalhead__txt">
            <h3>{info.name}</h3>
            {pal.nickname && pal.nickname !== info.name && <div className="ch-modalnick">“{pal.nickname}”</div>}
            <div className="ch-modalmeta">
              <span>Lv {pal.level}</span>
              {genderSymbol(pal.gender) && <span>{genderSymbol(pal.gender)}</span>}
              {pal.rank > 0 && <span className="palcard__rank">★{pal.rank} Condenser</span>}
              {pal.is_boss && <span className="pbadge pbadge--alpha">ALPHA</span>}
              {pal.is_lucky && <span className="pbadge pbadge--lucky">LUCKY</span>}
            </div>
            <div className="palcard__elems">
              {info.elements.map((el) => (
                <span key={el} className="elem" style={{ color: elementColor(el), background: `${elementColor(el)}22` }}>
                  {el}
                </span>
              ))}
            </div>
          </div>
        </div>

        <div className="ch-modalgrid">
          <ModalStat label="HP" value={`${pal.hp}${pal.max_hp ? ` / ${pal.max_hp}` : ""}`} />
          <ModalStat label="Sanity" value={pal.sanity} />
          <ModalStat label="Hunger" value={pal.stomach} />
          <ModalStat label="Friendship" value={pal.friendship_point} />
        </div>

        <section className="ch-section">
          <h4>IVs (Talents)</h4>
          <div className="ch-ivrow">
            <IvBar label="HP" v={pal.talent_hp} />
            <IvBar label="ATK" v={pal.talent_shot} />
            <IvBar label="DEF" v={pal.talent_defense} />
          </div>
        </section>

        <section className="ch-section">
          <h4>Souls</h4>
          <div className="ch-souls">
            {souls.map(([label, v]) => (
              <div key={label} className="ch-soul">
                <span className="ch-soul__l">{label}</span>
                <span className="ch-soul__v">+{v}</span>
              </div>
            ))}
          </div>
        </section>

        {pal.active_skills.length > 0 && (
          <ChipSection title="Active Skills" chips={pal.active_skills.map(skillName)} />
        )}
        {pal.passive_skills.length > 0 && (
          <ChipSection title="Passive Skills" chips={pal.passive_skills.map(passiveName)} accent />
        )}
        {work.length > 0 && (
          <section className="ch-section">
            <h4>Work Suitability</h4>
            <div className="ch-work">
              {work.map(([code, rank]) => (
                <div key={code} className="ch-workrow">
                  <span>{workLabel(code)}</span>
                  <span className="ch-worklv">Lv {rank}</span>
                </div>
              ))}
            </div>
          </section>
        )}
      </div>
    </div>
  );
}

function ModalStat({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="ch-mstat">
      <span className="ch-mstat__v">{value}</span>
      <span className="ch-mstat__l">{label}</span>
    </div>
  );
}

function ChipSection({ title, chips, accent }: { title: string; chips: string[]; accent?: boolean }) {
  return (
    <section className="ch-section">
      <h4>{title}</h4>
      <div className="ch-chips">
        {chips.map((c, i) => (
          <span key={`${c}-${i}`} className={`ch-skill${accent ? " ch-skill--passive" : ""}`}>
            {c}
          </span>
        ))}
      </div>
    </section>
  );
}

function InventoryView({
  inventory,
  itemName,
}: {
  inventory: ItemContainer[];
  itemName: (id: string) => string;
}) {
  const groups = inventory
    .map((c) => ({ c, filled: c.slots.filter((s) => s.static_id) }))
    .filter((g) => g.filled.length > 0);

  if (groups.length === 0) {
    return <p className="ch-empty">Empty — or stored in a per-player save that isn’t on disk.</p>;
  }

  return (
    <div className="ch-inv">
      {groups.map(({ c, filled }) => (
        <div key={c.id} className="ch-invgroup">
          <div className="ch-invhead">
            {humanize(c.container_type)} <span>{filled.length}</span>
          </div>
          <div className="ch-slots">
            {filled.map((s) => (
              <div key={s.slot_index} className="ch-slot" title={itemName(s.static_id)}>
                <ItemIcon staticId={s.static_id} size={30} />
                <span className="ch-slot__name">
                  {itemName(s.static_id)}
                  {s.dynamic_item?.egg_params ? " 🥚" : ""}
                </span>
                <span className="ch-slot__count">×{s.count}</span>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
