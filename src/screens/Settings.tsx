import "./settings.css";
import { useMemo, useState } from "react";
import {
  Copy,
  Download,
  Eye,
  EyeOff,
  Info,
  Lock,
  RotateCcw,
  Save,
  Search,
  ShieldAlert,
  TriangleAlert,
} from "lucide-react";
import { TopBar } from "../components/TopBar";
import { Button } from "../components/Button";
import { Field, Input } from "../components/Field";
import { Skeleton } from "../components/Skeleton";
import { EmptyState } from "../components/EmptyState";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { ConfirmSpec } from "../components/ConfirmDialog";
import { useSettings } from "../hooks/queries";
import { useBridge, useSettingsIni, useWriteSettingsIni } from "../hooks/bridge";
import {
  GROUP_ORDER,
  groupOf,
  isPasswordKey,
  isSensitiveKey,
  labelFor,
} from "../lib/settingsSchema";
import type { Group } from "../lib/settingsSchema";
import { useToast } from "../hooks/useToast";
import type { Settings as SettingsMap } from "../types/api";
import type { SettingsIni, SettingsIniEntry } from "../types/bridge";

type Value = SettingsMap[string];
type Row = [key: string, value: Value];

function renderValue(value: Value) {
  if (typeof value === "boolean") {
    return value ? (
      <span className="chip chip--good">
        <span className="chip__dot" />
        On
      </span>
    ) : (
      <span className="pill set-off">Off</span>
    );
  }
  if (typeof value === "number") {
    return <span className="set-mono">{value}</span>;
  }
  const str = String(value);
  return <span className="set-mono">{str.trim() === "" ? "—" : str}</span>;
}

export function Settings() {
  const bridge = useBridge();
  const ini = useSettingsIni(bridge.available);

  // When the bridge exposes a readable PalWorldSettings.ini, edit it directly.
  if (bridge.available && ini.data) {
    return <SettingsEditor ini={ini.data} refetch={() => ini.refetch()} fetching={ini.isFetching} />;
  }
  // Otherwise fall back to the read-only REST inspector.
  return <SettingsInspector />;
}

/* ---------------- Editable mode (via the bridge) ---------------- */

type CtrlKind = "bool" | "number" | "text";

function controlKind(e: SettingsIniEntry): CtrlKind {
  if (e.value === "True" || e.value === "False") return "bool";
  if (!e.quoted && /^-?\d+(\.\d+)?$/.test(e.value.trim())) return "number";
  return "text";
}

function SettingsEditor({
  ini,
  refetch,
  fetching,
}: {
  ini: SettingsIni;
  refetch: () => void;
  fetching: boolean;
}) {
  const toast = useToast();
  const write = useWriteSettingsIni();
  const [query, setQuery] = useState("");
  const [edits, setEdits] = useState<Record<string, string>>({});
  const [reveal, setReveal] = useState<Record<string, boolean>>({});
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [confirm, setConfirm] = useState<ConfirmSpec | null>(null);

  const writable = ini.writable;
  const orig = useMemo(() => {
    const m = new Map<string, SettingsIniEntry>();
    for (const e of ini.settings) m.set(e.key, e);
    return m;
  }, [ini.settings]);

  const changed = useMemo(() => {
    const out: Record<string, string> = {};
    for (const [k, v] of Object.entries(edits)) {
      const e = orig.get(k);
      if (e && v !== e.value) out[k] = v;
    }
    return out;
  }, [edits, orig]);
  const dirtyKeys = Object.keys(changed);
  const touchesSensitive = dirtyKeys.some(isSensitiveKey);

  const valueOf = (e: SettingsIniEntry) => edits[e.key] ?? e.value;
  const setValue = (key: string, v: string) => setEdits((s) => ({ ...s, [key]: v }));

  // Group non-sensitive settings by schema; sensitive keys go to Advanced.
  const { groups, advanced } = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const match = (e: SettingsIniEntry) =>
      !needle || labelFor(e.key).toLowerCase().includes(needle) || e.key.toLowerCase().includes(needle);
    const buckets = new Map<Group, SettingsIniEntry[]>();
    const adv: SettingsIniEntry[] = [];
    for (const e of ini.settings) {
      if (!match(e)) continue;
      if (isSensitiveKey(e.key)) {
        adv.push(e);
        continue;
      }
      const g = groupOf(e.key);
      const arr = buckets.get(g) ?? [];
      arr.push(e);
      buckets.set(g, arr);
    }
    const byLabel = (a: SettingsIniEntry, b: SettingsIniEntry) =>
      labelFor(a.key).localeCompare(labelFor(b.key));
    return {
      groups: GROUP_ORDER.filter((g) => buckets.has(g)).map((g) => ({
        group: g,
        rows: buckets.get(g)!.sort(byLabel),
      })),
      advanced: adv.sort(byLabel),
    };
  }, [ini.settings, query]);

  const doSave = () => {
    write.mutate(changed, {
      onSuccess: (res) => {
        setEdits({});
        toast.success(
          "Settings saved",
          `${dirtyKeys.length} change${dirtyKeys.length === 1 ? "" : "s"} written${
            res.requires_restart ? " — restart the server to apply." : "."
          }`,
        );
        refetch();
      },
      onError: (e) => toast.error("Save failed", e instanceof Error ? e.message : String(e)),
    });
  };

  const onSave = () => {
    if (!writable || dirtyKeys.length === 0) return;
    if (touchesSensitive) {
      const names = dirtyKeys.filter(isSensitiveKey).map(labelFor).join(", ");
      setConfirm({
        title: "Change network / auth settings?",
        body: `You're changing ${names}. A wrong value here (ports, REST API, or the admin password) can lock this app out of the server. A backup is taken first, and changes only apply after a restart.`,
        confirmText: "Write changes",
        onConfirm: async () => doSave(),
      });
      return;
    }
    doSave();
  };

  const renderControl = (e: SettingsIniEntry) => {
    const kind = controlKind(e);
    const v = valueOf(e);
    const dirty = e.key in changed;
    if (!writable) {
      const shown = isPasswordKey(e.key) && v ? "••••••••" : v || "—";
      return <span className="set-mono set-ro">{shown}</span>;
    }
    if (kind === "bool") {
      const on = v === "True";
      return (
        <button
          type="button"
          className={`set-switch${on ? " is-on" : ""}${dirty ? " is-dirty" : ""}`}
          role="switch"
          aria-checked={on}
          onClick={() => setValue(e.key, on ? "False" : "True")}
        >
          <span className="set-switch__knob" />
        </button>
      );
    }
    const isPw = isPasswordKey(e.key);
    return (
      <span className="set-inputwrap">
        <input
          className={`set-input${dirty ? " is-dirty" : ""}`}
          type={isPw && !reveal[e.key] ? "password" : kind === "number" ? "number" : "text"}
          value={v}
          onChange={(ev) => setValue(e.key, ev.target.value)}
          spellCheck={false}
        />
        {isPw && (
          <button
            type="button"
            className="set-reveal"
            onClick={() => setReveal((r) => ({ ...r, [e.key]: !r[e.key] }))}
            aria-label={reveal[e.key] ? "Hide" : "Reveal"}
          >
            {reveal[e.key] ? <EyeOff size={14} /> : <Eye size={14} />}
          </button>
        )}
      </span>
    );
  };

  const Rows = ({ rows }: { rows: SettingsIniEntry[] }) => (
    <div className="set-rows">
      {rows.map((e) => (
        <div key={e.key} className="set-row">
          <span className="set-row__label">
            {labelFor(e.key)}
            {isSensitiveKey(e.key) && <ShieldAlert size={12} className="set-row__warn" />}
          </span>
          <span className="set-row__val set-row__val--edit">{renderControl(e)}</span>
        </div>
      ))}
    </div>
  );

  const shown = groups.reduce((n, g) => n + g.rows.length, 0) + advanced.length;

  return (
    <>
      <TopBar breadcrumb="Control" title="Settings" showLive={false} onRefresh={refetch} refreshing={fetching} />
      <div className="page">
        <div className="card card--pad set-note">
          <div className="set-note__body">
            <span className="set-note__ic">
              <Info size={18} />
            </span>
            <p>
              Editing <span className="set-mono">PalWorldSettings.ini</span> via the bridge.
              {writable ? " Changes take effect after a server restart." : ""}
              <span className="set-note__path"> {ini.path}</span>
            </p>
          </div>
          {!writable && (
            <span className="set-locked">
              <Lock size={13} /> Read-only — enable save edits in the PSM Bridge
            </span>
          )}
        </div>

        <Field label="Search settings" hint={`${shown} shown`}>
          <Input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Filter by name or key…" />
        </Field>

        {shown === 0 ? (
          <EmptyState icon={Search} title="No matching settings" detail={`Nothing matches “${query.trim()}”.`} />
        ) : (
          <>
            {groups.map(({ group, rows }) => (
              <div key={group} className="card card--pad set-group">
                <div className="eyebrow">{group}</div>
                <Rows rows={rows} />
              </div>
            ))}

            {advanced.length > 0 && (
              <div className="card card--pad set-group set-adv">
                <button className="set-adv__head" onClick={() => setShowAdvanced((s) => !s)}>
                  <span className="eyebrow set-adv__title">
                    <ShieldAlert size={13} /> Advanced — network &amp; auth
                  </span>
                  <span className="set-adv__toggle">{showAdvanced ? "Hide" : "Show"}</span>
                </button>
                {showAdvanced && (
                  <>
                    <p className="set-adv__warn">
                      Changing ports, the REST API toggle, or passwords can lock this app out of the
                      server. Passwords are masked; edits are confirmed before saving.
                    </p>
                    <Rows rows={advanced} />
                  </>
                )}
              </div>
            )}
          </>
        )}

        {writable && dirtyKeys.length > 0 && (
          <div className="set-savebar">
            <span className="set-savebar__count">
              {dirtyKeys.length} unsaved change{dirtyKeys.length === 1 ? "" : "s"}
              {touchesSensitive && <span className="set-savebar__flag"> · includes network/auth</span>}
            </span>
            <div className="set-savebar__actions">
              <Button variant="ghost" size="sm" onClick={() => setEdits({})} disabled={write.isPending}>
                <RotateCcw size={14} /> Discard
              </Button>
              <Button variant="primary" size="sm" onClick={onSave} loading={write.isPending}>
                <Save size={15} /> Save changes
              </Button>
            </div>
          </div>
        )}
      </div>
      <ConfirmDialog spec={confirm} onClose={() => setConfirm(null)} />
    </>
  );
}

/* ---------------- Read-only mode (REST API) ---------------- */

function SettingsInspector() {
  const q = useSettings();
  const toast = useToast();
  const [query, setQuery] = useState("");

  const s = q.data;

  const groups = useMemo<{ group: Group; rows: Row[] }[]>(() => {
    if (!s) return [];
    const needle = query.trim().toLowerCase();
    const buckets = new Map<Group, Row[]>();
    for (const [key, value] of Object.entries(s)) {
      if (needle && !labelFor(key).toLowerCase().includes(needle) && !key.toLowerCase().includes(needle)) continue;
      const g = groupOf(key);
      const arr = buckets.get(g) ?? [];
      arr.push([key, value]);
      buckets.set(g, arr);
    }
    return GROUP_ORDER.filter((g) => buckets.has(g)).map((g) => ({
      group: g,
      rows: buckets.get(g)!.sort((a, b) => labelFor(a[0]).localeCompare(labelFor(b[0]))),
    }));
  }, [s, query]);

  const total = s ? Object.keys(s).length : 0;
  const shown = groups.reduce((n, g) => n + g.rows.length, 0);

  const exportJson = () => {
    if (!s) return;
    const blob = new Blob([JSON.stringify(s, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "settings.json";
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    toast.success("Exported", "settings.json");
  };

  const copy = (value: Value) => {
    navigator.clipboard.writeText(String(value));
    toast.info("Copied");
  };

  return (
    <>
      <TopBar breadcrumb="Control" title="Settings" showLive={false} onRefresh={() => q.refetch()} refreshing={q.isFetching} />
      <div className="page">
        {q.isError ? (
          <EmptyState
            icon={TriangleAlert}
            tone="error"
            title="Can’t reach the server"
            detail="The REST API didn’t respond. Check the server is running and RESTAPIEnabled=True."
          />
        ) : q.isLoading && !s ? (
          <>
            <Skeleton height={92} radius="var(--r-lg)" />
            <Skeleton height={64} radius="var(--r-md)" />
            <Skeleton height={280} radius="var(--r-lg)" />
          </>
        ) : (
          <>
            <div className="card card--pad set-note">
              <div className="set-note__body">
                <span className="set-note__ic">
                  <Info size={18} />
                </span>
                <p>
                  These values come straight from the server (read-only). Connect the PSM Bridge to
                  edit <span className="set-mono">PalWorldSettings.ini</span> directly.
                </p>
              </div>
              <Button variant="primary" size="sm" onClick={exportJson} disabled={!s}>
                <Download size={15} /> Export JSON
              </Button>
            </div>

            <Field label="Search settings" hint={total ? `${shown} of ${total} shown` : undefined}>
              <Input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Filter by name or key…" />
            </Field>

            {shown === 0 ? (
              <EmptyState icon={Search} title="No matching settings" detail={`Nothing matches “${query.trim()}”. Try a different term.`} />
            ) : (
              groups.map(({ group, rows }) => (
                <div key={group} className="card card--pad set-group">
                  <div className="eyebrow">{group}</div>
                  <div className="set-rows">
                    {rows.map(([key, value]) => (
                      <div key={key} className="set-row">
                        <span className="set-row__label">{labelFor(key)}</span>
                        <span className="set-row__val">{renderValue(value)}</span>
                        <button
                          className="icobtn set-row__copy"
                          onClick={() => copy(value)}
                          aria-label={`Copy ${labelFor(key)}`}
                          title="Copy value"
                        >
                          <Copy size={14} />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              ))
            )}
          </>
        )}
      </div>
    </>
  );
}
