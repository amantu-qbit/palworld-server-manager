import "./characters.css";
import { useEffect, useMemo, useState } from "react";
import {
  Copy,
  Cpu,
  PackageOpen,
  PawPrint,
  Pencil,
  Plus,
  Search,
  TriangleAlert,
  Users,
  X,
} from "lucide-react";
import { TopBar } from "../components/TopBar";
import { EmptyState } from "../components/EmptyState";
import { Skeleton } from "../components/Skeleton";
import { PalIcon } from "../components/PalIcon";
import { ItemIcon } from "../components/ItemIcon";
import { Button } from "../components/Button";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { ConfirmSpec } from "../components/ConfirmDialog";
import { Drawer } from "../components/Drawer";
import { Field, Input } from "../components/Field";
import {
  useBridge,
  useBridgeGuilds,
  useBridgePlayerDetail,
  useBridgePlayers,
  useBridgeReference,
  useEditPal,
  useEditPlayer,
  useEditPlayerTechnologies,
} from "../hooks/bridge";
import { useToast } from "../hooks/useToast";
import { TechTree } from "../components/TechTree";
import { elementColor, isRare, palInfo, wazaElement } from "../lib/palDex";
import { humanize, statusLabel, workLabel } from "../lib/palLabels";
import { EXP_TABLE, LEVEL_CAP, MAX_LEVEL, levelProgress } from "../lib/expTable";
import { DISABLED_PASSIVES, passiveRank, passiveRankColor } from "../lib/skillMeta";
import type { TechMeta } from "../lib/techDex";
import type {
  EditPalBody,
  EditPlayerBody,
  ItemContainer,
  Pal,
  PlayerDetail,
  PlayerSummary,
} from "../types/bridge";

const genderSymbol = (g: string) => {
  const l = g.toLowerCase();
  return l.includes("female") ? "♀" : l.includes("male") ? "♂" : "";
};
const ivColor = (v: number) => (v >= 90 ? "#3ad19a" : v >= 60 ? "#e6b450" : "#7c8494");

const clampInt = (v: unknown, min: number, max: number, fallback = min) => {
  const n = Math.trunc(Number(v));
  return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : fallback;
};

const progressTitle = (p: { into: number; next: number | null }) =>
  p.next != null ? `${p.into.toLocaleString()} / ${p.next.toLocaleString()} to next` : "Max level";

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
  const bridge = useBridge();
  const toast = useToast();
  const items = useBridgeReference("items");
  const active = useBridgeReference("active_skills");
  const passive = useBridgeReference("passive_skills");
  const [tab, setTab] = useState<Tab>("pals");
  const [selectedPal, setSelectedPal] = useState<Pal | null>(null);

  const canEdit = bridge.writesEnabled && !bridge.serverRunning;
  const d = detail.data;
  const itemName = (id: string) => items.data?.[id] ?? humanize(id);
  const skillName = (code: string) => active.data?.[code] ?? humanize(code);
  const passiveName = (code: string) => passive.data?.[code] ?? humanize(code);
  const guild = (summary.guild_id && guildName.get(summary.guild_id)) || "No guild";

  const copy = (text: string) => {
    navigator.clipboard.writeText(text).then(
      () => toast.success("Copied"),
      () => toast.error("Copy failed"),
    );
  };

  const groups = useMemo(() => groupPals(d), [d]);

  return (
    <>
      <header className="ch-head">
        <div className="ch-headrow">
          <h2>{summary.nickname || "(unnamed)"}</h2>
          <button className="ch-copy" onClick={() => copy(summary.uid)} title="Copy UID" aria-label="Copy UID">
            <Copy size={12} />
          </button>
        </div>
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

          {tab === "character" && <CharacterTab detail={d} canEdit={canEdit} serverRunning={bridge.serverRunning} />}

          {tab === "inventory" && <InventoryView inventory={d.inventory} itemName={itemName} />}
        </>
      ) : null}

      {selectedPal && (
        <PalDetailModal
          pal={selectedPal}
          onClose={() => setSelectedPal(null)}
          skillName={skillName}
          passiveName={passiveName}
          canEdit={canEdit}
          activeCatalog={active.data}
          passiveCatalog={passive.data}
          onCopy={copy}
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

function CharacterTab({
  detail,
  canEdit,
  serverRunning,
}: {
  detail: PlayerDetail;
  canEdit: boolean;
  serverRunning: boolean;
}) {
  const stats = { ...detail.status_points, ...detail.ext_status_points };
  const toast = useToast();
  const editTech = useEditPlayerTechnologies();
  const [editOpen, setEditOpen] = useState(false);
  const [confirm, setConfirm] = useState<ConfirmSpec | null>(null);

  const prog = levelProgress(detail.exp, detail.level, false);

  const toggleTech = (m: TechMeta, on: boolean) => {
    setConfirm({
      title: on ? `Relock ${m.name}?` : `Unlock ${m.name}?`,
      body: on
        ? "Removes the technology from this player's unlocked list. Spent points are not refunded."
        : `Adds ${m.name} to this player's unlocked technologies.`,
      confirmText: on ? "Relock" : "Unlock",
      danger: on,
      onConfirm: async () => {
        try {
          await editTech.mutateAsync({
            uid: detail.summary.uid,
            body: on ? { relock: [m.code] } : { unlock: [m.code] },
          });
          toast.success(on ? "Technology relocked" : "Technology unlocked", "Backup saved before the change.");
        } catch (e) {
          toast.error("Edit failed", e instanceof Error ? e.message : String(e));
        }
      },
    });
  };

  return (
    <div className="ch-char">
      {(canEdit || serverRunning) && (
        <div className="ch-charhead">
          {serverRunning && <span className="ch-editnote">Stop the server to edit saves</span>}
          <Button size="sm" variant="ghost" onClick={() => setEditOpen(true)} disabled={!canEdit}>
            <Pencil size={13} /> Edit
          </Button>
        </div>
      )}
      <div className="ch-statgrid">
        <Stat label="Level" value={detail.level} bar={{ pct: prog.pct, title: progressTitle(prog) }} />
        <Stat label="EXP" value={detail.exp.toLocaleString()} />
        {Object.entries(stats).map(([k, v]) => (
          <Stat key={k} label={statusLabel(k)} value={`+${v}`} />
        ))}
      </div>

      <TechTree
        unlocked={detail.technologies}
        techPoints={detail.technology_points}
        ancientPoints={detail.boss_technology_points}
        editable={canEdit}
        onToggle={toggleTech}
      />

      <PlayerEditDrawer open={editOpen} onClose={() => setEditOpen(false)} detail={detail} />
      <ConfirmDialog spec={confirm} onClose={() => setConfirm(null)} />
    </div>
  );
}

function Stat({
  label,
  value,
  bar,
}: {
  label: string;
  value: string | number;
  bar?: { pct: number; title: string };
}) {
  return (
    <div className="ch-stat">
      <span className="ch-stat__v">{value}</span>
      <span className="ch-stat__l">{label}</span>
      {bar && (
        <span className="ch-lvlbar" title={bar.title}>
          <i style={{ width: `${Math.min(100, Math.max(0, bar.pct))}%` }} />
        </span>
      )}
    </div>
  );
}

/** Changed status-point entries only (keys stay the on-disk names). */
function diffPoints(
  original: Record<string, number>,
  edited: Record<string, string>,
): Record<string, number> | null {
  const out: Record<string, number> = {};
  for (const [k, v] of Object.entries(edited)) {
    const n = clampInt(v, 0, 9999, original[k] ?? 0);
    if (n !== original[k]) out[k] = n;
  }
  return Object.keys(out).length ? out : null;
}

const toStrings = (m: Record<string, number>) =>
  Object.fromEntries(Object.entries(m).map(([k, v]) => [k, String(v)]));

function PlayerEditDrawer({
  open,
  onClose,
  detail,
}: {
  open: boolean;
  onClose: () => void;
  detail: PlayerDetail;
}) {
  const toast = useToast();
  const editPlayer = useEditPlayer();
  const editTech = useEditPlayerTechnologies();

  const [level, setLevel] = useState(String(detail.level));
  const [exp, setExp] = useState(String(detail.exp));
  const [advanced, setAdvanced] = useState(false);
  const [status, setStatus] = useState<Record<string, string>>(() => toStrings(detail.status_points));
  const [ext, setExt] = useState<Record<string, string>>(() => toStrings(detail.ext_status_points));
  const [tp, setTp] = useState(String(detail.technology_points));
  const [btp, setBtp] = useState(String(detail.boss_technology_points));

  useEffect(() => {
    if (!open) return;
    setLevel(String(detail.level));
    setExp(String(detail.exp));
    setAdvanced(false);
    setStatus(toStrings(detail.status_points));
    setExt(toStrings(detail.ext_status_points));
    setTp(String(detail.technology_points));
    setBtp(String(detail.boss_technology_points));
  }, [open, detail]);

  const onLevel = (v: string) => {
    setLevel(v);
    const n = Number(v);
    if (Number.isInteger(n) && n >= 1 && n <= MAX_LEVEL) setExp(String(EXP_TABLE.total[n - 1]));
  };

  const busy = editPlayer.isPending || editTech.isPending;

  const save = async () => {
    const lv = clampInt(level, 1, MAX_LEVEL, detail.level);
    const xp = clampInt(exp, 0, Number.MAX_SAFE_INTEGER, detail.exp);
    const body: EditPlayerBody = {};
    if (lv !== detail.level) body.level = lv;
    if (xp !== detail.exp) body.exp = xp;
    const sp = diffPoints(detail.status_points, status);
    if (sp) body.status_points = sp;
    const ep = diffPoints(detail.ext_status_points, ext);
    if (ep) body.ext_status_points = ep;
    const tpN = clampInt(tp, 0, 99999, detail.technology_points);
    const btpN = clampInt(btp, 0, 99999, detail.boss_technology_points);
    const techChanged = tpN !== detail.technology_points || btpN !== detail.boss_technology_points;

    if (!Object.keys(body).length && !techChanged) {
      onClose();
      return;
    }
    try {
      if (Object.keys(body).length) {
        await editPlayer.mutateAsync({ uid: detail.summary.uid, body });
      }
      if (techChanged) {
        await editTech.mutateAsync({
          uid: detail.summary.uid,
          body: { technology_point: tpN, boss_technology_point: btpN },
        });
      }
      toast.success("Character updated", "Backup saved before the change.");
      onClose();
    } catch (e) {
      toast.error("Save failed", e instanceof Error ? e.message : String(e));
    }
  };

  const pointFields = (
    map: Record<string, string>,
    set: (m: Record<string, string>) => void,
  ) =>
    Object.keys(map).map((k) => (
      <Field key={k} label={statusLabel(k)}>
        <Input
          mono
          type="number"
          min={0}
          max={9999}
          value={map[k]}
          onChange={(e) => set({ ...map, [k]: e.target.value })}
        />
      </Field>
    ));

  return (
    <Drawer
      open={open}
      onClose={onClose}
      title="Edit character"
      subtitle={detail.summary.nickname || "(unnamed)"}
    >
      <div className="ch-form">
        <div className="ch-formrow">
          <Field label="Level" hint={`1.0 level cap: ${LEVEL_CAP} — values up to ${MAX_LEVEL} accepted`}>
            <Input
              mono
              type="number"
              min={1}
              max={MAX_LEVEL}
              value={level}
              onChange={(e) => onLevel(e.target.value)}
            />
          </Field>
          <Field
            label="EXP"
            hint={
              <button type="button" className="ch-adv" onClick={() => setAdvanced((a) => !a)}>
                {advanced ? "Auto-sync from level" : "Edit raw value"}
              </button>
            }
          >
            <Input
              mono
              type="number"
              min={0}
              value={exp}
              onChange={(e) => setExp(e.target.value)}
              disabled={!advanced}
            />
          </Field>
        </div>

        {Object.keys(status).length > 0 && (
          <section className="ch-formsec">
            <div className="eyebrow">Status points</div>
            <div className="ch-formgrid">{pointFields(status, setStatus)}</div>
          </section>
        )}
        {Object.keys(ext).length > 0 && (
          <section className="ch-formsec">
            <div className="eyebrow">Extended status points</div>
            <div className="ch-formgrid">{pointFields(ext, setExt)}</div>
          </section>
        )}

        <section className="ch-formsec">
          <div className="eyebrow">Technology</div>
          <div className="ch-formrow">
            <Field label="Tech points">
              <Input mono type="number" min={0} value={tp} onChange={(e) => setTp(e.target.value)} />
            </Field>
            <Field label="Ancient points">
              <Input mono type="number" min={0} value={btp} onChange={(e) => setBtp(e.target.value)} />
            </Field>
          </div>
        </section>

        <div className="ch-formactions">
          <Button variant="primary" onClick={save} loading={busy}>
            Save changes
          </Button>
          <Button variant="ghost" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
        </div>
      </div>
    </Drawer>
  );
}

function PalDetailModal({
  pal,
  onClose,
  skillName,
  passiveName,
  canEdit,
  activeCatalog,
  passiveCatalog,
  onCopy,
}: {
  pal: Pal;
  onClose: () => void;
  skillName: (c: string) => string;
  passiveName: (c: string) => string;
  canEdit: boolean;
  activeCatalog: Record<string, string> | undefined;
  passiveCatalog: Record<string, string> | undefined;
  onCopy: (text: string) => void;
}) {
  const [edit, setEdit] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const info = palInfo(pal.character_id);
  const accent = info.elements.length ? elementColor(info.elements[0]) : "#5a6070";
  const prog = levelProgress(pal.exp, pal.level, true);
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
        <div className="ch-modaltools">
          {canEdit && (
            <button
              className={`ch-modalclose ch-modaledit${edit ? " is-on" : ""}`}
              onClick={() => setEdit((e) => !e)}
              title={edit ? "Stop editing" : "Edit Pal"}
              aria-label={edit ? "Stop editing" : "Edit Pal"}
            >
              <Pencil size={14} />
            </button>
          )}
          <button className="ch-modalclose" onClick={onClose} aria-label="Close">
            <X size={16} />
          </button>
        </div>
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
            <div className="ch-lvlbar ch-lvlbar--wide" title={progressTitle(prog)}>
              <i style={{ width: `${prog.pct}%` }} />
            </div>
            <div className="palcard__elems">
              {info.elements.map((el) => (
                <span key={el} className="elem" style={{ color: elementColor(el), background: `${elementColor(el)}22` }}>
                  {el}
                </span>
              ))}
            </div>
            <div className="ch-modalid">
              <span title={pal.instance_id}>{pal.instance_id}</span>
              <button className="ch-copy" onClick={() => onCopy(pal.instance_id)} title="Copy instance id" aria-label="Copy instance id">
                <Copy size={11} />
              </button>
            </div>
          </div>
        </div>

        {edit ? (
          <PalEditForm
            pal={pal}
            activeCatalog={activeCatalog}
            passiveCatalog={passiveCatalog}
            skillName={skillName}
            passiveName={passiveName}
            onCancel={() => setEdit(false)}
            onSaved={onClose}
          />
        ) : (
          <>
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
          </>
        )}
      </div>
    </div>
  );
}

const sameList = (a: string[], b: string[]) => a.length === b.length && a.every((v, i) => v === b[i]);

function PalEditForm({
  pal,
  activeCatalog,
  passiveCatalog,
  skillName,
  passiveName,
  onCancel,
  onSaved,
}: {
  pal: Pal;
  activeCatalog: Record<string, string> | undefined;
  passiveCatalog: Record<string, string> | undefined;
  skillName: (c: string) => string;
  passiveName: (c: string) => string;
  onCancel: () => void;
  onSaved: () => void;
}) {
  const toast = useToast();
  const editPal = useEditPal();

  const [level, setLevel] = useState(String(pal.level));
  const [exp, setExp] = useState(String(pal.exp));
  const [advanced, setAdvanced] = useState(false);
  const [nickname, setNickname] = useState(pal.nickname);
  const [rank, setRank] = useState(pal.rank);
  const [soulHp, setSoulHp] = useState(pal.rank_hp);
  const [soulAtk, setSoulAtk] = useState(pal.rank_attack);
  const [soulDef, setSoulDef] = useState(pal.rank_defense);
  const [soulCraft, setSoulCraft] = useState(pal.rank_craftspeed);
  const [talHp, setTalHp] = useState(String(pal.talent_hp));
  const [talShot, setTalShot] = useState(String(pal.talent_shot));
  const [talDef, setTalDef] = useState(String(pal.talent_defense));
  const [passives, setPassives] = useState<string[]>(pal.passive_skills);
  const [actives, setActives] = useState<string[]>(pal.active_skills);

  const onLevel = (v: string) => {
    setLevel(v);
    const n = Number(v);
    if (Number.isInteger(n) && n >= 1 && n <= MAX_LEVEL) setExp(String(EXP_TABLE.palTotal[n - 1]));
  };

  const save = async () => {
    const lv = clampInt(level, 1, MAX_LEVEL, pal.level);
    const xp = clampInt(exp, 0, Number.MAX_SAFE_INTEGER, pal.exp);
    const body: EditPalBody = {};
    if (lv !== pal.level) body.level = lv;
    if (xp !== pal.exp) body.exp = xp;
    if (nickname !== pal.nickname) body.nickname = nickname;
    if (rank !== pal.rank) body.rank = rank;
    if (soulHp !== pal.rank_hp) body.rank_hp = soulHp;
    if (soulAtk !== pal.rank_attack) body.rank_attack = soulAtk;
    if (soulDef !== pal.rank_defense) body.rank_defense = soulDef;
    if (soulCraft !== pal.rank_craftspeed) body.rank_craftspeed = soulCraft;
    const tHp = clampInt(talHp, 0, 100, pal.talent_hp);
    const tShot = clampInt(talShot, 0, 100, pal.talent_shot);
    const tDef = clampInt(talDef, 0, 100, pal.talent_defense);
    if (tHp !== pal.talent_hp) body.talent_hp = tHp;
    if (tShot !== pal.talent_shot) body.talent_shot = tShot;
    if (tDef !== pal.talent_defense) body.talent_defense = tDef;
    if (!sameList(passives, pal.passive_skills)) body.passive_skills = passives;
    if (!sameList(actives, pal.active_skills)) body.active_skills = actives;

    if (!Object.keys(body).length) {
      onCancel();
      return;
    }
    try {
      await editPal.mutateAsync({ instanceId: pal.instance_id, body });
      toast.success("Pal updated", "Backup saved before the change.");
      onSaved();
    } catch (e) {
      toast.error("Save failed", e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="ch-form">
      <div className="ch-formrow">
        <Field label="Level" hint={`Cap ${LEVEL_CAP}; up to ${MAX_LEVEL} accepted`}>
          <Input mono type="number" min={1} max={MAX_LEVEL} value={level} onChange={(e) => onLevel(e.target.value)} />
        </Field>
        <Field
          label="EXP"
          hint={
            <button type="button" className="ch-adv" onClick={() => setAdvanced((a) => !a)}>
              {advanced ? "Auto-sync from level" : "Edit raw value"}
            </button>
          }
        >
          <Input mono type="number" min={0} value={exp} onChange={(e) => setExp(e.target.value)} disabled={!advanced} />
        </Field>
      </div>

      <Field label="Nickname">
        <Input value={nickname} onChange={(e) => setNickname(e.target.value)} placeholder={palInfo(pal.character_id).name} />
      </Field>

      <section className="ch-formsec">
        <div className="eyebrow">Souls (0–20)</div>
        <div className="ch-steprow">
          <Stepper label="HP" v={soulHp} min={0} max={20} onChange={setSoulHp} />
          <Stepper label="Attack" v={soulAtk} min={0} max={20} onChange={setSoulAtk} />
          <Stepper label="Defense" v={soulDef} min={0} max={20} onChange={setSoulDef} />
          <Stepper label="Craft" v={soulCraft} min={0} max={20} onChange={setSoulCraft} />
        </div>
      </section>

      <section className="ch-formsec">
        <div className="eyebrow">Condenser rank (0–5)</div>
        <div className="ch-steprow">
          <Stepper label="Rank" v={rank} min={0} max={5} onChange={setRank} />
        </div>
      </section>

      <section className="ch-formsec">
        <div className="eyebrow">Talents (0–100)</div>
        <div className="ch-formgrid ch-formgrid--3">
          <Field label="HP">
            <Input mono type="number" min={0} max={100} value={talHp} onChange={(e) => setTalHp(e.target.value)} />
          </Field>
          <Field label="Attack">
            <Input mono type="number" min={0} max={100} value={talShot} onChange={(e) => setTalShot(e.target.value)} />
          </Field>
          <Field label="Defense">
            <Input mono type="number" min={0} max={100} value={talDef} onChange={(e) => setTalDef(e.target.value)} />
          </Field>
        </div>
      </section>

      <SkillSlots
        label="Passive skills"
        max={4}
        values={passives}
        catalog={passiveCatalog}
        exclude={DISABLED_PASSIVES}
        name={passiveName}
        accent={(id) => passiveRankColor(passiveRank(id))}
        onChange={setPassives}
      />

      <SkillSlots
        label="Active skills"
        max={3}
        values={actives}
        catalog={activeCatalog}
        name={skillName}
        chip={(id) => {
          const el = wazaElement(id);
          return el ? { el, color: elementColor(el) } : null;
        }}
        onChange={setActives}
      />

      <div className="ch-formactions">
        <Button variant="primary" onClick={save} loading={editPal.isPending}>
          Save changes
        </Button>
        <Button variant="ghost" onClick={onCancel} disabled={editPal.isPending}>
          Cancel
        </Button>
      </div>
    </div>
  );
}

function Stepper({
  label,
  v,
  min,
  max,
  onChange,
}: {
  label: string;
  v: number;
  min: number;
  max: number;
  onChange: (n: number) => void;
}) {
  return (
    <div className="ch-stepwrap">
      <span className="ch-step__l">{label}</span>
      <div className="ch-step">
        <button type="button" onClick={() => onChange(Math.max(min, v - 1))} disabled={v <= min} aria-label={`Decrease ${label}`}>
          −
        </button>
        <b>{v}</b>
        <button type="button" onClick={() => onChange(Math.min(max, v + 1))} disabled={v >= max} aria-label={`Increase ${label}`}>
          +
        </button>
      </div>
    </div>
  );
}

const PICK_CAP = 120;

function SkillSlots({
  label,
  max,
  values,
  catalog,
  exclude,
  name,
  accent,
  chip,
  onChange,
}: {
  label: string;
  max: number;
  values: string[];
  catalog: Record<string, string> | undefined;
  exclude?: Set<string>;
  name: (id: string) => string;
  accent?: (id: string) => string;
  chip?: (id: string) => { el: string; color: string } | null;
  onChange: (v: string[]) => void;
}) {
  const [search, setSearch] = useState("");
  const [picking, setPicking] = useState(false);
  const q = search.trim().toLowerCase();

  const options = useMemo(() => {
    if (!catalog) return [] as [string, string][];
    const chosen = new Set(values);
    const rows: [string, string][] = [];
    for (const [id, nm] of Object.entries(catalog)) {
      if (exclude?.has(id) || chosen.has(id)) continue;
      if (q && !nm.toLowerCase().includes(q) && !id.toLowerCase().includes(q)) continue;
      rows.push([id, nm]);
    }
    rows.sort((a, b) => a[1].localeCompare(b[1]));
    return rows;
  }, [catalog, values, q, exclude]);

  const add = (id: string) => {
    const next = [...values, id];
    onChange(next);
    if (next.length >= max) setPicking(false);
  };

  return (
    <section className="ch-formsec">
      <div className="eyebrow">
        {label} · {values.length}/{max}
      </div>
      <div className="ch-slotchips">
        {values.map((id) => {
          const c = chip?.(id) ?? null;
          return (
            <span
              key={id}
              className="ch-slotchip"
              style={accent ? { borderLeftColor: accent(id) } : undefined}
              title={id}
            >
              {name(id)}
              {c && (
                <i className="elem" style={{ color: c.color, background: `${c.color}22` }}>
                  {c.el}
                </i>
              )}
              <button onClick={() => onChange(values.filter((v) => v !== id))} aria-label={`Remove ${name(id)}`}>
                <X size={11} />
              </button>
            </span>
          );
        })}
        {values.length < max && (
          <button className={`ch-slotadd${picking ? " is-on" : ""}`} onClick={() => setPicking((p) => !p)}>
            <Plus size={12} /> Add
          </button>
        )}
      </div>
      {picking && values.length < max && (
        <div className="ch-pickpanel">
          <div className="ch-picksearch">
            <Search size={12} />
            <input value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Search…" autoFocus />
          </div>
          <div className="ch-picklist">
            {options.slice(0, PICK_CAP).map(([id, nm]) => {
              const c = chip?.(id) ?? null;
              return (
                <button
                  key={id}
                  className="ch-pickrow"
                  style={accent ? { borderLeftColor: accent(id) } : undefined}
                  onClick={() => add(id)}
                  title={id}
                >
                  <span className="ch-pickname">{nm}</span>
                  {c ? (
                    <i className="elem" style={{ color: c.color, background: `${c.color}22` }}>
                      {c.el}
                    </i>
                  ) : (
                    <span className="ch-pickid">{id.replace(/^EPalWazaID::/, "")}</span>
                  )}
                </button>
              );
            })}
            {options.length > PICK_CAP && (
              <div className="ch-pickmore">
                Showing {PICK_CAP} of {options.length} — refine your search.
              </div>
            )}
            {options.length === 0 && <div className="ch-pickmore">No matches.</div>}
          </div>
        </div>
      )}
    </section>
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
