import "./guilds.css";
import { useEffect, useMemo, useState } from "react";
import type { CSSProperties } from "react";
import {
  Castle,
  ChevronsUp,
  HeartPulse,
  PawPrint,
  Save,
  Sparkles,
  TriangleAlert,
  Users,
} from "lucide-react";
import { TopBar } from "../components/TopBar";
import { Button } from "../components/Button";
import { EmptyState } from "../components/EmptyState";
import { Field, Input } from "../components/Field";
import { Skeleton } from "../components/Skeleton";
import { ContainerPane } from "./Storage";
import { chestName } from "../lib/containers";
import {
  useBridge,
  useBridgeContainers,
  useBridgeGuilds,
  useEditBase,
  useEditBasePals,
  useEditGuild,
  useHealBasePals,
} from "../hooks/bridge";
import { useToast } from "../hooks/useToast";
import { EXP_TABLE } from "../lib/expTable";
import type { Base, ContainerInfo, EditBaseBody, EditGuildBody, Guild } from "../types/bridge";

const AREA_MIN = 1750;
const AREA_MAX = 35000;
const AREA_STEP = 250;
const AREA_DEFAULT = 3500;
const LVL_MIN = 1;
const LVL_MAX = 30;

/**
 * The 13 Palworld work suitabilities as their fully-qualified on-disk enum
 * values. These MUST match how the save stores them ("EPalWorkSuitability::X")
 * — a bare suffix would never match an existing entry and the bridge would
 * reject it, so keep the prefix.
 */
const WORK_SUITABILITIES = [
  "EPalWorkSuitability::EmitFlame",
  "EPalWorkSuitability::Watering",
  "EPalWorkSuitability::Seeding",
  "EPalWorkSuitability::GenerateElectricity",
  "EPalWorkSuitability::Handcraft",
  "EPalWorkSuitability::Collection",
  "EPalWorkSuitability::Deforest",
  "EPalWorkSuitability::Mining",
  "EPalWorkSuitability::OilExtraction",
  "EPalWorkSuitability::ProductMedicine",
  "EPalWorkSuitability::Cool",
  "EPalWorkSuitability::Transport",
  "EPalWorkSuitability::MonsterFarm",
];
const MAX_WORK_RANK = 5;

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));
const guildName = (g: Guild) => g.name?.trim() || "Unnamed Guild";
const clampArea = (n: number) => Math.min(AREA_MAX, Math.max(AREA_MIN, n));
const plural = (n: number, s: string) => `${n} ${s}${n === 1 ? "" : "s"}`;

export function Guilds() {
  const bridge = useBridge();
  const guildsQ = useBridgeGuilds();
  const containersQ = useBridgeContainers();
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const guilds = guildsQ.data ?? [];
  useEffect(() => {
    if (guilds.length && (!selectedId || !guilds.some((g) => g.id === selectedId))) {
      setSelectedId(guilds[0].id);
    }
  }, [guilds, selectedId]);

  const selected = guilds.find((g) => g.id === selectedId) ?? null;
  // The selected guild's chest + base-storage containers (from /v1/containers),
  // so we can reuse Storage's ContainerPane verbatim for every storage editor.
  const guildContainers = useMemo(
    () => (containersQ.data ?? []).filter((c) => c.guild_id === selectedId),
    [containersQ.data, selectedId],
  );
  const chest = useMemo(
    () => guildContainers.find((c) => c.kind === "guild_chest") ?? null,
    [guildContainers],
  );

  return (
    <>
      <TopBar
        breadcrumb="Server+"
        title="Guilds"
        onRefresh={() => {
          guildsQ.refetch();
          containersQ.refetch();
        }}
        refreshing={guildsQ.isFetching || containersQ.isFetching}
      />
      <div className="page gl-page">
        {guildsQ.isError ? (
          <EmptyState
            icon={TriangleAlert}
            tone="error"
            title="Can’t reach the bridge"
            detail={(guildsQ.error as Error)?.message ?? "The bridge didn’t respond."}
          />
        ) : guildsQ.isLoading && !guildsQ.data ? (
          <div className="card card--pad col" style={{ gap: 12 }}>
            {Array.from({ length: 4 }).map((_, i) => (
              <Skeleton key={i} height={52} radius="var(--r-md)" />
            ))}
          </div>
        ) : guilds.length === 0 ? (
          <EmptyState icon={Castle} title="No guilds found" detail="This save has no guilds yet." />
        ) : (
          <div className="gl">
            <aside className="gl-list">
              {guilds.map((g) => (
                <button
                  key={g.id}
                  className={`gl-grow${g.id === selectedId ? " is-active" : ""}`}
                  onClick={() => setSelectedId(g.id)}
                >
                  <span className="gl-grow__name">{guildName(g)}</span>
                  <span className="gl-grow__meta">
                    <Users size={11} /> {g.players.length} · Lv {g.base_camp_level} ·{" "}
                    {plural(g.bases.length, "base")}
                  </span>
                </button>
              ))}
            </aside>

            <main className="gl-main">
              {selected ? (
                <GuildDetail
                  key={selected.id}
                  guild={selected}
                  chest={chest}
                  containers={guildContainers}
                  writesEnabled={bridge.writesEnabled}
                  serverRunning={bridge.serverRunning}
                />
              ) : (
                <div className="gl-empty">
                  <Castle size={26} />
                  <p>Select a guild to inspect and edit it.</p>
                </div>
              )}
            </main>
          </div>
        )}
      </div>
    </>
  );
}

function GuildDetail({
  guild,
  chest,
  containers,
  writesEnabled,
  serverRunning,
}: {
  guild: Guild;
  chest: ContainerInfo | null;
  containers: ContainerInfo[];
  writesEnabled: boolean;
  serverRunning: boolean;
}) {
  const toast = useToast();
  const editGuild = useEditGuild();
  const canEdit = writesEnabled && !serverRunning;

  const [name, setName] = useState(guild.name);
  const [level, setLevel] = useState(guild.base_camp_level);
  useEffect(() => {
    setName(guild.name);
    setLevel(guild.base_camp_level);
  }, [guild.id, guild.name, guild.base_camp_level]);

  const dirty = name !== guild.name || level !== guild.base_camp_level;

  const save = async () => {
    const body: EditGuildBody = {};
    if (name.trim() && name !== guild.name) body.guild_name = name.trim();
    if (level !== guild.base_camp_level) body.base_camp_level = level;
    if (Object.keys(body).length === 0) return;
    try {
      await editGuild.mutateAsync({ id: guild.id, body });
      toast.success("Guild saved", "Backup saved before the change.");
    } catch (e) {
      toast.error("Save failed", errMsg(e));
    }
  };

  return (
    <div className="gl-detail">
      <header className="gl-head">
        <div className="gl-head__id">
          <Castle size={18} />
          <h2>{guildName(guild)}</h2>
        </div>
        <div className="gl-head__meta">
          <span className="gl-chip">{plural(guild.players.length, "member")}</span>
          <span className="gl-chip gl-chip--dim">Lv {guild.base_camp_level}</span>
          <span className="gl-chip gl-chip--dim">{plural(guild.bases.length, "base")}</span>
        </div>
      </header>

      {!writesEnabled && (
        <div className="gl-note">Read-only — enable writes in the psm-bridge config</div>
      )}
      {writesEnabled && serverRunning && (
        <div className="gl-note gl-note--warn">Stop the server to edit saves</div>
      )}

      <section className="card card--pad gl-card">
        <div className="gl-card__title">Guild</div>
        <div className="gl-form">
          <Field label="Guild name">
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={!canEdit}
              maxLength={64}
              placeholder="Unnamed Guild"
            />
          </Field>
          <SliderField
            label="Base camp level"
            min={LVL_MIN}
            max={LVL_MAX}
            step={1}
            value={Math.min(LVL_MAX, Math.max(LVL_MIN, level))}
            onChange={setLevel}
            disabled={!canEdit}
            display={`Lv ${level}`}
            hint="1–30"
          />
        </div>
        {canEdit && (
          <div className="gl-actions">
            <Button variant="primary" onClick={save} disabled={!dirty} loading={editGuild.isPending}>
              <Save size={14} /> Save guild
            </Button>
          </div>
        )}
      </section>

      {guild.bases.length > 0 && (
        <section className="gl-bases">
          <div className="gl-card__title">Base camps · area, pals &amp; storage</div>
          <div className="gl-basestack">
            {guild.bases.map((b, i) => (
              <BaseSection
                key={b.id}
                base={b}
                index={i}
                canEdit={canEdit}
                writesEnabled={writesEnabled}
                serverRunning={serverRunning}
                storages={containers.filter(
                  (c) => c.kind === "base_storage" && b.storage_containers.includes(c.id),
                )}
              />
            ))}
          </div>
        </section>
      )}

      <section className="gl-chest">
        <div className="gl-card__title">Guild chest</div>
        {chest ? (
          <ContainerPane c={chest} writesEnabled={writesEnabled} serverRunning={serverRunning} />
        ) : (
          <div className="gl-chest__empty">This guild has no chest container in the save yet.</div>
        )}
      </section>
    </div>
  );
}

function BaseSection({
  base,
  index,
  canEdit,
  storages,
  writesEnabled,
  serverRunning,
}: {
  base: Base;
  index: number;
  canEdit: boolean;
  storages: ContainerInfo[];
  writesEnabled: boolean;
  serverRunning: boolean;
}) {
  const toast = useToast();
  const editBase = useEditBase();

  const decodedArea = base.area_range > 0 ? Math.round(base.area_range) : AREA_DEFAULT;
  const [name, setName] = useState(base.name);
  const [area, setArea] = useState(decodedArea);
  const [storeIdx, setStoreIdx] = useState(0);
  useEffect(() => {
    setName(base.name);
    setArea(base.area_range > 0 ? Math.round(base.area_range) : AREA_DEFAULT);
  }, [base.id, base.name, base.area_range]);

  const dirty = name !== base.name || area !== decodedArea;
  const store = storages[Math.min(storeIdx, Math.max(0, storages.length - 1))] ?? null;

  const save = async () => {
    const body: EditBaseBody = {};
    if (name.trim() && name !== base.name) body.name = name.trim();
    if (area !== decodedArea) body.area_range = area;
    if (Object.keys(body).length === 0) return;
    try {
      await editBase.mutateAsync({ id: base.id, body });
      toast.success("Base saved", "Backup saved before the change.");
    } catch (e) {
      toast.error("Save failed", errMsg(e));
    }
  };

  return (
    <div className="gl-base card card--pad">
      <div className="gl-base__head">
        <span className="gl-base__idx">Base {index + 1}</span>
        {base.position && (
          <span className="gl-base__pos" title="World position (x, y)">
            {Math.round(base.position[0])}, {Math.round(base.position[1])}
          </span>
        )}
      </div>

      <div className="gl-baseform">
        <Field label="Base name">
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            disabled={!canEdit}
            maxLength={64}
            placeholder="Unnamed base"
          />
        </Field>
        <SliderField
          label="Build area radius"
          min={AREA_MIN}
          max={AREA_MAX}
          step={AREA_STEP}
          value={clampArea(area)}
          onChange={setArea}
          disabled={!canEdit}
          display={area.toLocaleString()}
          hint="Vanilla default 3,500"
        />
      </div>
      {canEdit && (
        <div className="gl-actions">
          <Button
            size="sm"
            variant="primary"
            onClick={save}
            disabled={!dirty}
            loading={editBase.isPending}
          >
            <Save size={13} /> Save base
          </Button>
        </div>
      )}

      <div className="gl-subhead">Stationed pals</div>
      <BasePalControls base={base} canEdit={canEdit} />

      <div className="gl-subhead">Storage · {plural(storages.length, "chest")}</div>
      {storages.length > 0 ? (
        <>
          {storages.length > 1 && (
            <div className="gl-storetabs" role="tablist">
              {storages.map((s, i) => (
                <button
                  key={s.id}
                  role="tab"
                  aria-selected={i === storeIdx}
                  className={i === storeIdx ? "is-on" : ""}
                  onClick={() => setStoreIdx(i)}
                >
                  {chestName(s.object_name) ?? `Chest ${i + 1}`}
                  <span>
                    {s.used}/{s.slot_num}
                  </span>
                </button>
              ))}
            </div>
          )}
          {store && (
            <ContainerPane
              key={store.id}
              c={store}
              writesEnabled={writesEnabled}
              serverRunning={serverRunning}
            />
          )}
        </>
      ) : (
        <div className="gl-chest__empty">No built storage chests at this base.</div>
      )}
    </div>
  );
}

function BasePalControls({ base, canEdit }: { base: Base; canEdit: boolean }) {
  const toast = useToast();
  const healAll = useHealBasePals();
  const editAll = useEditBasePals();
  const [level, setLevel] = useState("50");

  const count = base.pals.length;
  if (count === 0) {
    return <p className="gl-nopals">No pals are stationed at this base.</p>;
  }

  const busy = healAll.isPending || editAll.isPending;

  const heal = async () => {
    try {
      await healAll.mutateAsync(base.id);
      toast.success("Base pals healed", `${plural(count, "pal")} restored — backup saved.`);
    } catch (e) {
      toast.error("Heal failed", errMsg(e));
    }
  };

  const levelAll = async () => {
    const lv = Math.min(100, Math.max(1, Math.trunc(Number(level)) || 1));
    // Pals use their own EXP curve (palTotal), which caps below the player max.
    const exp = EXP_TABLE.palTotal[Math.min(lv, EXP_TABLE.palTotal.length) - 1] ?? 0;
    try {
      await editAll.mutateAsync({ id: base.id, body: { level: lv, exp } });
      toast.success("Base pals leveled", `${plural(count, "pal")} set to level ${lv}.`);
    } catch (e) {
      toast.error("Level failed", errMsg(e));
    }
  };

  const maxWork = async () => {
    const work_suitability = Object.fromEntries(WORK_SUITABILITIES.map((k) => [k, MAX_WORK_RANK]));
    try {
      await editAll.mutateAsync({ id: base.id, body: { work_suitability } });
      toast.success("Work affinity maxed", `All work suitabilities +${MAX_WORK_RANK} on ${plural(count, "pal")}.`);
    } catch (e) {
      toast.error("Update failed", errMsg(e));
    }
  };

  return (
    <div className="gl-palops">
      <span className="gl-palops__count">
        <PawPrint size={13} /> {plural(count, "pal")} stationed
      </span>
      {canEdit && (
        <div className="gl-palops__btns">
          <Button size="sm" variant="ghost" onClick={heal} loading={healAll.isPending} disabled={busy}>
            <HeartPulse size={13} /> Heal all
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={maxWork}
            loading={editAll.isPending}
            disabled={busy}
            title="Set every work suitability to +5 on all base pals"
          >
            <Sparkles size={13} /> Max work affinity
          </Button>
          <div className="gl-levelall">
            <Input
              mono
              type="number"
              min={1}
              max={100}
              value={level}
              onChange={(e) => setLevel(e.target.value)}
              disabled={busy}
              aria-label="Level to apply to all base pals"
            />
            <Button size="sm" variant="ghost" onClick={levelAll} loading={editAll.isPending} disabled={busy}>
              <ChevronsUp size={13} /> Level all
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

function SliderField({
  label,
  min,
  max,
  step,
  value,
  onChange,
  disabled,
  display,
  hint,
}: {
  label: string;
  min: number;
  max: number;
  step: number;
  value: number;
  onChange: (n: number) => void;
  disabled: boolean;
  display: string;
  hint?: string;
}) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div className={`gl-slider${disabled ? " is-disabled" : ""}`}>
      <div className="gl-slider__top">
        <span className="gl-slider__label">{label}</span>
        <span className="gl-slider__val">{display}</span>
      </div>
      <input
        type="range"
        className="gl-range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{ "--fill": `${pct}%` } as CSSProperties}
      />
      {hint && <span className="gl-slider__hint">{hint}</span>}
    </div>
  );
}
