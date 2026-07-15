import "./rawsave.css";
import { useEffect, useMemo, useState } from "react";
import { ChevronRight, Copy, FileSearch, Search, TriangleAlert } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { EmptyState } from "../components/EmptyState";
import { Skeleton } from "../components/Skeleton";
import { useBridgeSavFiles, useBridgeSavTree } from "../hooks/bridge";
import type { SavNode } from "../types/bridge";

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

type Obj = { [k: string]: SavNode };
const isObj = (n: SavNode): n is Obj => typeof n === "object" && n !== null && !Array.isArray(n);
const nodeType = (n: SavNode): string | null => (isObj(n) && typeof n._type === "string" ? n._type : null);

const INLINE_TYPES = new Set(["Enum", "Byte", "Vector", "Quat", "LinearColor", "Color", "DateTime"]);

/** A short one-line rendering for a leaf node, or null if the node is expandable. */
function inlineValue(n: SavNode): string | null {
  if (n === null) return "null";
  if (typeof n === "string") return `"${n.length > 80 ? n.slice(0, 80) + "…" : n}"`;
  if (typeof n === "number" || typeof n === "boolean") return String(n);
  if (Array.isArray(n)) return null;
  if ("_bytes" in n) return `bytes · ${fmtBytes(Number(n._bytes))}`;
  const t = nodeType(n);
  if (t === "Enum" || t === "Byte") return `${n.value}`;
  if (t === "Vector") return `(${n.x}, ${n.y}, ${n.z})`;
  if (t === "Quat") return `(${n.x}, ${n.y}, ${n.z}, ${n.w})`;
  if (t === "LinearColor" || t === "Color") return `rgba(${n.r}, ${n.g}, ${n.b}, ${n.a})`;
  if (t === "DateTime") return `${n.ticks} ticks`;
  return null;
}

const isContainer = (n: SavNode): boolean => isObj(n) && "_count" in n;
const isExpandable = (n: SavNode): boolean =>
  isObj(n) &&
  !("_bytes" in n) &&
  !(nodeType(n) && INLINE_TYPES.has(nodeType(n)!)) &&
  (isContainer(n) || Object.keys(n).some((k) => !k.startsWith("_")));

const joinPath = (base: string, seg: string) => (base ? `${base}.${seg}` : seg);

interface Child {
  label: string;
  path: string;
  node: SavNode;
}

/** The addressable children of an expanded node, with each child's drill path. */
function childrenOf(node: SavNode, path: string): Child[] {
  if (!isObj(node)) return [];
  const items = node.items;
  if (Array.isArray(items)) {
    const t = nodeType(node) ?? "";
    return items.map((item, i) => {
      if (t.startsWith("Map<") && isObj(item)) {
        const keyLabel = inlineValue(item.key) ?? "{…}";
        return { label: `${i} · ${keyLabel}`, path: `${joinPath(path, String(i))}.value`, node: item.value };
      }
      return { label: `${i}`, path: joinPath(path, String(i)), node: item };
    });
  }
  return Object.keys(node)
    .filter((k) => !k.startsWith("_"))
    .map((k) => ({ label: k, path: joinPath(path, k), node: node[k] }));
}

export function RawSave() {
  const files = useBridgeSavFiles();
  const [file, setFile] = useState<string | null>(null);
  const [abs, setAbs] = useState("");

  useEffect(() => {
    if (!file && files.data?.length) {
      const level = files.data.find((f) => f.name === "Level.sav");
      setFile(level?.rel_path ?? files.data[0].rel_path);
    }
  }, [files.data, file]);

  return (
    <>
      <TopBar
        breadcrumb="Server+"
        title="Raw Save"
        onRefresh={() => files.refetch()}
        refreshing={files.isFetching}
      />
      <div className="page rs-page">
        {files.isError ? (
          <EmptyState
            icon={TriangleAlert}
            tone="error"
            title="Can’t reach the bridge"
            detail={(files.error as Error)?.message ?? "The bridge didn’t respond."}
          />
        ) : (
          <div className="rs">
            <aside className="rs-list">
              <form
                className="rs-abs"
                onSubmit={(e) => {
                  e.preventDefault();
                  if (abs.trim()) setFile(abs.trim());
                }}
              >
                <input
                  value={abs}
                  onChange={(e) => setAbs(e.target.value)}
                  placeholder="Absolute path to a .sav (e.g. LocalData.sav)…"
                  spellCheck={false}
                />
                <button type="submit">Load</button>
              </form>
              <div className="rs-files">
                {files.isLoading && !files.data
                  ? Array.from({ length: 6 }).map((_, i) => <Skeleton key={i} height={40} radius="var(--r-sm)" />)
                  : (files.data ?? []).map((f) => (
                      <button
                        key={f.rel_path}
                        className={`rs-frow${f.rel_path === file ? " is-active" : ""}`}
                        onClick={() => setFile(f.rel_path)}
                        title={f.rel_path}
                      >
                        <span className="rs-frow__name">{f.name}</span>
                        <span className="rs-frow__meta">
                          {f.rel_path} · {fmtBytes(f.size_bytes)}
                        </span>
                      </button>
                    ))}
                {files.data && files.data.length === 0 && (
                  <div className="rs-nomatch">No .sav files under the save dir.</div>
                )}
              </div>
            </aside>

            <main className="rs-detail">
              {file ? (
                <SavTree key={file} file={file} />
              ) : (
                <div className="rs-detail__empty">
                  <FileSearch size={26} />
                  <p>Pick a .sav file to inspect its full decoded structure.</p>
                </div>
              )}
            </main>
          </div>
        )}
      </div>
    </>
  );
}

function SavTree({ file }: { file: string }) {
  const q = useBridgeSavTree(file, "", 200, 2);
  const [filter, setFilter] = useState("");
  const fq = filter.trim().toLowerCase();

  const roots = useMemo(() => {
    if (!q.data) return [];
    const all = childrenOf(q.data.node, "");
    return fq ? all.filter((c) => c.label.toLowerCase().includes(fq)) : all;
  }, [q.data, fq]);

  return (
    <>
      <header className="rs-head">
        <h2 title={file}>{file}</h2>
        <div className="rs-head__meta">
          {q.data && <span className="rs-chip">{fmtBytes(q.data.meta.size_bytes)}</span>}
          {q.data && (
            <button
              className="rs-copy"
              onClick={() => navigator.clipboard?.writeText(JSON.stringify(q.data!.node, null, 2))}
              title="Copy this subtree as JSON"
            >
              <Copy size={12} /> JSON
            </button>
          )}
        </div>
      </header>

      <div className="rs-filter">
        <Search size={13} />
        <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Filter top-level keys…" />
      </div>

      {q.isLoading ? (
        <div className="col" style={{ gap: 6 }}>
          {Array.from({ length: 8 }).map((_, i) => (
            <Skeleton key={i} height={26} radius="var(--r-xs)" />
          ))}
        </div>
      ) : q.isError ? (
        <p className="rs-err">{(q.error as Error)?.message ?? "Failed to load this .sav."}</p>
      ) : (
        <div className="rs-tree">
          {roots.map((c) => (
            <NodeRow key={c.path} file={file} label={c.label} path={c.path} node={c.node} depth={0} />
          ))}
          {roots.length === 0 && <div className="rs-nomatch">No keys match “{filter}”.</div>}
        </div>
      )}
    </>
  );
}

function NodeRow({
  file,
  label,
  path,
  node,
  depth,
}: {
  file: string;
  label: string;
  path: string;
  node: SavNode;
  depth: number;
}) {
  const [open, setOpen] = useState(false);
  const inline = inlineValue(node);
  const expandable = isExpandable(node);
  const collapsed = isObj(node) && node._collapsed === true;
  const type = nodeType(node);

  // A collapsed container is re-fetched (with its own depth budget) on expand.
  const sub = useBridgeSavTree(collapsed && open ? file : null, path);
  const effective = collapsed ? sub.data?.node ?? null : node;
  // Count/truncation come from the resolved node once fetched, else the stub.
  const shape = effective ?? node;
  const count = isObj(shape) && typeof shape._count === "number" ? shape._count : null;
  const truncated = isObj(shape) && shape._truncated === true;
  const children = open && effective ? childrenOf(effective, path) : [];

  if (!expandable) {
    return (
      <div className="rs-row" style={{ paddingLeft: depth * 14 }}>
        <span className="rs-key">{label}</span>
        {type && !INLINE_TYPES.has(type) ? null : type && <span className="rs-badge">{type}</span>}
        <span className="rs-val">{inline ?? "—"}</span>
      </div>
    );
  }

  return (
    <div className="rs-node">
      <button className="rs-row rs-row--btn" style={{ paddingLeft: depth * 14 }} onClick={() => setOpen((o) => !o)}>
        <ChevronRight size={13} className={`rs-caret${open ? " is-open" : ""}`} />
        <span className="rs-key">{label}</span>
        {type && <span className="rs-badge">{type}</span>}
        {count != null && (
          <span className="rs-count">
            {count}
            {truncated ? "+" : ""}
          </span>
        )}
      </button>
      {open && (
        <div className="rs-children">
          {collapsed && sub.isLoading && <div className="rs-loading">Loading…</div>}
          {collapsed && sub.isError && <div className="rs-err">{(sub.error as Error)?.message}</div>}
          {truncated && (
            <div className="rs-trunc" style={{ paddingLeft: (depth + 1) * 14 }}>
              showing first {children.length} of {count}
            </div>
          )}
          {children.map((c) => (
            <NodeRow key={c.path} file={file} label={c.label} path={c.path} node={c.node} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  );
}
