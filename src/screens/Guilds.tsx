import "./guilds.css";
import { useEffect, useMemo, useState } from "react";
import type { CSSProperties } from "react";
import { Castle, Save, TriangleAlert, Users } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { Button } from "../components/Button";
import { EmptyState } from "../components/EmptyState";
import { Field, Input } from "../components/Field";
import { Skeleton } from "../components/Skeleton";
import { ContainerPane } from "./Storage";
import {
  useBridge,
  useBridgeContainers,
  useBridgeGuilds,
  useEditBase,
  useEditGuild,
} from "../hooks/bridge";
import { useToast } from "../hooks/useToast";
import type {
  Base,
  ContainerInfo,
  EditBaseBody,
  EditGuildBody,
  Guild,
} from "../types/bridge";

const AREA_MIN = 1750;
const AREA_MAX = 35000;
const AREA_STEP = 250;
const AREA_DEFAULT = 3500;
const LVL_MIN = 1;
const LVL_MAX = 30;

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));
const guildName = (g: Guild) => g.name?.trim() || "Unnamed Guild";
const clampArea = (n: number) => Math.min(AREA_MAX, Math.max(AREA_MIN, n));

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
  // The guild chest as a labeled container (from /v1/containers), so we can
  // reuse Storage's ContainerPane verbatim for the chest editor.
  const chest = useMemo(
    () =>
      (containersQ.data ?? []).find(
        (c) => c.kind === "guild_chest" && c.guild_id === selectedId,
      ) ?? null,
    [containersQ.data, selectedId],
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
                    {g.bases.length} base{g.bases.length === 1 ? "" : "s"}
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
  writesEnabled,
  serverRunning,
}: {
  guild: Guild;
  chest: ContainerInfo | null;
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
          <span className="gl-chip">
            {guild.players.length} member{guild.players.length === 1 ? "" : "s"}
          </span>
          <span className="gl-chip gl-chip--dim">Lv {guild.base_camp_level}</span>
          <span className="gl-chip gl-chip--dim">
            {guild.bases.length} base{guild.bases.length === 1 ? "" : "s"}
          </span>
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
          <div className="gl-card__title">Base camps · build area &amp; name</div>
          <div className="gl-basegrid">
            {guild.bases.map((b, i) => (
              <BaseCard key={b.id} base={b} index={i} canEdit={canEdit} />
            ))}
          </div>
        </section>
      )}

      <section className="gl-chest">
        <div className="gl-card__title">Guild chest</div>
        {chest ? (
          <ContainerPane c={chest} writesEnabled={writesEnabled} serverRunning={serverRunning} />
        ) : (
          <div className="gl-chest__empty">
            This guild has no chest container in the save yet.
          </div>
        )}
      </section>
    </div>
  );
}

function BaseCard({ base, index, canEdit }: { base: Base; index: number; canEdit: boolean }) {
  const toast = useToast();
  const editBase = useEditBase();

  const decodedArea = base.area_range > 0 ? Math.round(base.area_range) : AREA_DEFAULT;
  const [name, setName] = useState(base.name);
  const [area, setArea] = useState(decodedArea);
  useEffect(() => {
    setName(base.name);
    setArea(base.area_range > 0 ? Math.round(base.area_range) : AREA_DEFAULT);
  }, [base.id, base.name, base.area_range]);

  const dirty = name !== base.name || area !== decodedArea;

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
