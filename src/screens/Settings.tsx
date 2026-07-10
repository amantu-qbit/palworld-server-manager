import "./settings.css";
import { useMemo, useState } from "react";
import { Copy, Download, Info, Search, TriangleAlert } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { Button } from "../components/Button";
import { Field, Input } from "../components/Field";
import { Skeleton } from "../components/Skeleton";
import { EmptyState } from "../components/EmptyState";
import { useSettings } from "../hooks/queries";
import { GROUP_ORDER, groupOf, labelFor } from "../lib/settingsSchema";
import type { Group } from "../lib/settingsSchema";
import { useToast } from "../hooks/useToast";
import type { Settings as SettingsMap } from "../types/api";

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
      <TopBar
        breadcrumb="Control"
        title="Settings"
        showLive={false}
        onRefresh={() => q.refetch()}
        refreshing={q.isFetching}
      />
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
                  These values come straight from the server. The REST API is read-only for settings — editing
                  PalWorldSettings.ini directly is on the roadmap.
                </p>
              </div>
              <Button variant="primary" size="sm" onClick={exportJson} disabled={!s}>
                <Download size={15} /> Export JSON
              </Button>
            </div>

            <Field
              label="Search settings"
              hint={total ? `${shown} of ${total} shown` : undefined}
            >
              <Input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Filter by name or key…"
              />
            </Field>

            {shown === 0 ? (
              <EmptyState
                icon={Search}
                title="No matching settings"
                detail={`Nothing matches “${query.trim()}”. Try a different term.`}
              />
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
