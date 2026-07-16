import "./storage.css";
import { useEffect, useMemo, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Boxes,
  Egg,
  Eraser,
  Expand,
  Search,
  Shield,
  Sword,
  TriangleAlert,
} from "lucide-react";
import { TopBar } from "../components/TopBar";
import { Button } from "../components/Button";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { ConfirmSpec } from "../components/ConfirmDialog";
import { Drawer } from "../components/Drawer";
import { EmptyState } from "../components/EmptyState";
import { Field, Input } from "../components/Field";
import { ItemIcon } from "../components/ItemIcon";
import { Skeleton } from "../components/Skeleton";
import { bridgeApi } from "../api/bridge";
import {
  invalidateBridgeData,
  useBridge,
  useBridgeContainers,
  useBridgeReference,
  useResizeContainer,
  useSetContainerSlot,
} from "../hooks/bridge";
import { useToast } from "../hooks/useToast";
import {
  containerLabel,
  containerOwner,
  groupContainers,
  occupiedSlots,
  slotsDeletedByResize,
} from "../lib/containers";
import { DISABLED_ITEMS } from "../lib/skillMeta";
import { humanize } from "../lib/palLabels";
import type { ContainerInfo, DynamicItem, ItemContainerSlot } from "../types/bridge";

const clampCount = (n: number) => Math.min(9999, Math.max(1, Math.trunc(n) || 1));

/** Glyph for a slot that carries a dynamic payload (equipment / egg). */
function DynGlyph({ dyn }: { dyn: DynamicItem }) {
  const t = dyn.item_type.toLowerCase();
  const Icon = dyn.egg_params ? Egg : t.includes("armor") || t.includes("shield") ? Shield : Sword;
  return (
    <span className="st-cell__dyn" title={humanize(dyn.item_type)}>
      <Icon size={10} />
    </span>
  );
}

/** Small debounce for the picker search input. */
function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = window.setTimeout(() => setV(value), ms);
    return () => window.clearTimeout(t);
  }, [value, ms]);
  return v;
}

export function Storage() {
  const bridge = useBridge();
  const containers = useBridgeContainers();
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const data = containers.data;
  const groups = useMemo(() => groupContainers(data ?? []), [data]);

  useEffect(() => {
    if (data?.length && (!selectedId || !data.some((c) => c.id === selectedId))) {
      setSelectedId(groups[0]?.containers[0]?.id ?? null);
    }
  }, [data, groups, selectedId]);

  const selected = data?.find((c) => c.id === selectedId) ?? null;

  return (
    <>
      <TopBar
        breadcrumb="Server+"
        title="Storage"
        onRefresh={() => containers.refetch()}
        refreshing={containers.isFetching}
      />
      <div className="page st-page">
        {containers.isError ? (
          <EmptyState
            icon={TriangleAlert}
            tone="error"
            title="Can’t reach the bridge"
            detail={(containers.error as Error)?.message ?? "The bridge didn’t respond."}
          />
        ) : containers.isLoading && !data ? (
          <div className="card card--pad col" style={{ gap: 12 }}>
            {Array.from({ length: 5 }).map((_, i) => (
              <Skeleton key={i} height={48} radius="var(--r-md)" />
            ))}
          </div>
        ) : data && data.length === 0 ? (
          <EmptyState
            icon={Boxes}
            title="No storage found"
            detail="The save has no player inventories or guild chests yet."
          />
        ) : (
          <div className="st">
            <aside className="st-list">
              {groups.map((g) => (
                <div key={g.title} className="st-group">
                  <div className="st-group__head">{g.title}</div>
                  {g.containers.map((c) => {
                    const pct = c.slot_num > 0 ? Math.min(100, (c.used / c.slot_num) * 100) : 0;
                    return (
                      <button
                        key={c.id}
                        className={`st-row${c.id === selectedId ? " is-active" : ""}`}
                        onClick={() => setSelectedId(c.id)}
                      >
                        <span className="st-row__top">
                          <span className="st-row__label">{containerLabel(c)}</span>
                          <span className="st-row__fill">
                            {c.used}/{c.slot_num}
                          </span>
                        </span>
                        <span className="st-row__bar">
                          <i style={{ width: `${pct}%` }} />
                        </span>
                      </button>
                    );
                  })}
                </div>
              ))}
            </aside>

            <main className="st-main">
              {selected ? (
                <ContainerPane
                  key={selected.id}
                  c={selected}
                  writesEnabled={bridge.writesEnabled}
                  serverRunning={bridge.serverRunning}
                />
              ) : (
                <div className="st-main__empty">
                  <Boxes size={26} />
                  <p>Select a container to inspect its slots.</p>
                </div>
              )}
            </main>
          </div>
        )}
      </div>
    </>
  );
}

function ContainerPane({
  c,
  writesEnabled,
  serverRunning,
}: {
  c: ContainerInfo;
  writesEnabled: boolean;
  serverRunning: boolean;
}) {
  const items = useBridgeReference("items");
  const toast = useToast();
  const [editorSlot, setEditorSlot] = useState<number | null>(null);
  const [resizeOpen, setResizeOpen] = useState(false);
  const [confirm, setConfirm] = useState<ConfirmSpec | null>(null);
  const [clearing, setClearing] = useState<{ done: number; total: number } | null>(null);

  const canEdit = writesEnabled && !serverRunning;
  const itemName = (id: string) => items.data?.[id] ?? humanize(id);

  const slotMap = useMemo(() => {
    const m = new Map<number, ItemContainerSlot>();
    for (const s of occupiedSlots(c.slots)) m.set(s.slot_index, s);
    return m;
  }, [c.slots]);

  const runClear = async (occ: ItemContainerSlot[]) => {
    setClearing({ done: 0, total: occ.length });
    try {
      for (let i = 0; i < occ.length; i++) {
        await bridgeApi.setContainerSlot(c.id, occ[i].slot_index, "None", 0);
        setClearing({ done: i + 1, total: occ.length });
      }
      toast.success("Container cleared", `${occ.length} stacks removed — backup saved.`);
    } catch (e) {
      toast.error("Clear stopped partway", e instanceof Error ? e.message : String(e));
    } finally {
      setClearing(null);
      invalidateBridgeData();
    }
  };

  const askClear = () => {
    const occ = occupiedSlots(c.slots);
    if (!occ.length) return;
    setConfirm({
      title: "Clear this container?",
      body: (
        <>
          Removes all <b>{occ.length}</b> item stack{occ.length === 1 ? "" : "s"} from{" "}
          <b>{containerLabel(c)}</b>. A backup of the save is written before the first change.
        </>
      ),
      confirmText: "Clear container",
      danger: true,
      onConfirm: () => {
        void runClear(occ);
      },
    });
  };

  return (
    <>
      <header className="st-head">
        <div className="st-head__txt">
          <h2>{containerLabel(c)}</h2>
          <div className="st-head__meta">
            <span className="st-chip">{containerOwner(c)}</span>
            <span className="st-chip st-chip--dim">
              {c.used}/{c.slot_num} slots
            </span>
          </div>
        </div>
        <div className="st-tools">
          {clearing ? (
            <span className="st-clearing">
              Clearing {clearing.done}/{clearing.total}…
            </span>
          ) : writesEnabled ? (
            <>
              <Button size="sm" variant="ghost" onClick={() => setResizeOpen(true)} disabled={serverRunning}>
                <Expand size={14} /> Resize
              </Button>
              <Button
                size="sm"
                variant="ghost"
                className="st-dangerghost"
                onClick={askClear}
                disabled={serverRunning || c.used === 0}
              >
                <Eraser size={14} /> Clear container
              </Button>
            </>
          ) : null}
        </div>
      </header>

      {!writesEnabled && <div className="st-note">Read-only — enable writes in psm-bridge config</div>}
      {writesEnabled && serverRunning && (
        <div className="st-note st-note--warn">Stop the server to edit saves</div>
      )}

      <div className={`st-grid${clearing ? " st-grid--busy" : ""}`}>
        {Array.from({ length: c.slot_num }, (_, i) => {
          const s = slotMap.get(i);
          const clickable = !!s || canEdit;
          return (
            <button
              key={i}
              className={`st-cell${s ? " is-filled" : ""}`}
              disabled={!clickable}
              onClick={() => setEditorSlot(i)}
              title={s ? `${itemName(s.static_id)} ×${s.count}` : `Slot ${i} — empty`}
            >
              {s && <ItemIcon staticId={s.static_id} size={40} />}
              {s && <span className="st-cell__n">{s.count}</span>}
              {s?.dynamic_item && <DynGlyph dyn={s.dynamic_item} />}
            </button>
          );
        })}
      </div>

      <Drawer
        open={editorSlot != null}
        onClose={() => setEditorSlot(null)}
        title={editorSlot != null ? `Slot ${editorSlot}` : ""}
        subtitle={`${containerLabel(c)} · ${containerOwner(c)}`}
      >
        {editorSlot != null && (
          <SlotEditor
            key={editorSlot}
            c={c}
            slotIndex={editorSlot}
            slot={slotMap.get(editorSlot) ?? null}
            itemName={itemName}
            catalog={items.data}
            canEdit={canEdit}
            onClose={() => setEditorSlot(null)}
          />
        )}
      </Drawer>

      <ResizeDialog c={c} open={resizeOpen} itemName={itemName} onClose={() => setResizeOpen(false)} />
      <ConfirmDialog spec={confirm} onClose={() => setConfirm(null)} />
    </>
  );
}

/* ---- Slot editor (drawer body) ---- */

const PICKER_CAP = 200;

function SlotEditor({
  c,
  slotIndex,
  slot,
  itemName,
  catalog,
  canEdit,
  onClose,
}: {
  c: ContainerInfo;
  slotIndex: number;
  slot: ItemContainerSlot | null;
  itemName: (id: string) => string;
  catalog: Record<string, string> | undefined;
  canEdit: boolean;
  onClose: () => void;
}) {
  const toast = useToast();
  const setSlot = useSetContainerSlot();
  const [staticId, setStaticId] = useState(slot?.static_id ?? "");
  const [count, setCount] = useState(String(slot?.count ?? 1));
  const [search, setSearch] = useState("");
  const q = useDebounced(search.trim().toLowerCase(), 150);

  const matches = useMemo(() => {
    if (!catalog) return { rows: [] as [string, string][], total: 0 };
    const all: [string, string][] = [];
    for (const [id, name] of Object.entries(catalog)) {
      if (DISABLED_ITEMS.has(id)) continue;
      if (q && !name.toLowerCase().includes(q) && !id.toLowerCase().includes(q)) continue;
      all.push([id, name]);
    }
    all.sort((a, b) => a[1].localeCompare(b[1]));
    return { rows: all.slice(0, PICKER_CAP), total: all.length };
  }, [catalog, q]);

  const dyn = slot?.dynamic_item ?? null;

  const save = async () => {
    if (!staticId) return;
    try {
      await setSlot.mutateAsync({
        cid: c.id,
        slotIndex,
        staticId,
        count: clampCount(Number(count)),
      });
      toast.success("Slot saved", "Backup saved before the change.");
      onClose();
    } catch (e) {
      toast.error("Save failed", e instanceof Error ? e.message : String(e));
    }
  };

  const clear = async () => {
    try {
      await setSlot.mutateAsync({ cid: c.id, slotIndex, staticId: "None", count: 0 });
      toast.success("Slot cleared", "Backup saved before the change.");
      onClose();
    } catch (e) {
      toast.error("Clear failed", e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="st-editor">
      {slot ? (
        <div className="st-cur">
          <ItemIcon staticId={slot.static_id} size={34} />
          <div className="st-cur__txt">
            <b>{itemName(slot.static_id)}</b>
            <span>
              {slot.static_id} · ×{slot.count}
            </span>
          </div>
        </div>
      ) : (
        <p className="st-editor__empty">This slot is empty.</p>
      )}

      {dyn && (
        <div className="st-dyn">
          <div className="st-dyn__rows">
            <span>Type</span>
            <b>{humanize(dyn.item_type)}</b>
            {dyn.durability > 0 && (
              <>
                <span>Durability</span>
                <b>{dyn.durability.toFixed(1)}</b>
              </>
            )}
            {dyn.remaining_bullets > 0 && (
              <>
                <span>Ammo</span>
                <b>{dyn.remaining_bullets}</b>
              </>
            )}
            {dyn.passive_skill_list.length > 0 && (
              <>
                <span>Passives</span>
                <b>{dyn.passive_skill_list.map(humanize).join(", ")}</b>
              </>
            )}
            {dyn.egg_params && (
              <>
                <span>Egg</span>
                <b>{dyn.egg_params.steps_remaining} steps remaining</b>
              </>
            )}
          </div>
          {canEdit && (
            <p className="st-dyn__warn">
              <TriangleAlert size={12} /> Saving replaces this payload with a plain item stack.
            </p>
          )}
        </div>
      )}

      {canEdit && (
        <>
          <div className="st-pick">
            <div className="st-pick__search">
              <Search size={13} />
              <input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search items…"
              />
            </div>
            <div className="st-pick__list">
              {matches.rows.map(([id, name]) => (
                <button
                  key={id}
                  className={`st-pick__row${id === staticId ? " is-active" : ""}`}
                  onClick={() => setStaticId(id)}
                >
                  <ItemIcon staticId={id} size={22} />
                  <span className="st-pick__name">{name}</span>
                  <span className="st-pick__id">{id}</span>
                </button>
              ))}
              {matches.total > PICKER_CAP && (
                <div className="st-pick__more">
                  Showing {PICKER_CAP} of {matches.total} — refine your search.
                </div>
              )}
              {matches.total === 0 && <div className="st-pick__more">No items match.</div>}
            </div>
          </div>

          <Field label="Count" hint="1–9999">
            <Input
              mono
              type="number"
              min={1}
              max={9999}
              value={count}
              onChange={(e) => setCount(e.target.value)}
              onBlur={() => setCount(String(clampCount(Number(count))))}
            />
          </Field>

          <div className="st-editor__actions">
            <Button variant="primary" onClick={save} disabled={!staticId} loading={setSlot.isPending}>
              Save slot
            </Button>
            {slot && (
              <Button variant="danger" onClick={clear} disabled={setSlot.isPending}>
                Clear slot
              </Button>
            )}
          </div>
        </>
      )}
    </div>
  );
}

/* ---- Resize dialog (ports palworld-save-pal PR #299) ---- */

function ResizeDialog({
  c,
  open,
  itemName,
  onClose,
}: {
  c: ContainerInfo;
  open: boolean;
  itemName: (id: string) => string;
  onClose: () => void;
}) {
  const toast = useToast();
  const resize = useResizeContainer();
  const [val, setVal] = useState(String(c.slot_num));
  const [ack, setAck] = useState(false);

  useEffect(() => {
    if (open) {
      setVal(String(c.slot_num));
      setAck(false);
    }
  }, [open, c.slot_num]);

  const num = Number(val);
  const valid = Number.isInteger(num) && num >= 0 && num <= 9999;
  const doomed = useMemo(
    () => (valid ? slotsDeletedByResize(c.slots, num) : []),
    [valid, c.slots, num],
  );
  const canConfirm = valid && num !== c.slot_num && (doomed.length === 0 || ack);

  const confirm = async () => {
    if (!canConfirm) return;
    try {
      await resize.mutateAsync({ cid: c.id, slotNum: num });
      toast.success("Container resized", "Backup saved before the change.");
      onClose();
    } catch (e) {
      toast.error("Resize failed", e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          className="scrim modal-wrap"
          style={{ zIndex: "var(--z-modal)" as unknown as number }}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={onClose}
        >
          <motion.div
            className="modal card st-resize"
            initial={{ opacity: 0, scale: 0.94, y: 16 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.96, y: 8 }}
            transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
            role="dialog"
            aria-label="Resize container"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="modal__title">Resize container</h2>
            <p className="st-resize__sub">
              {containerLabel(c)} currently has <b>{c.slot_num}</b> slots ({c.used} used).
            </p>

            <Field label="Slots" hint="0–9999 — growing keeps items; shrinking deletes overflow">
              <Input
                mono
                type="number"
                min={0}
                max={9999}
                value={val}
                onChange={(e) => setVal(e.target.value)}
                autoFocus
              />
            </Field>

            {doomed.length > 0 && (
              <div className="st-doomed">
                <div className="st-doomed__head">
                  <TriangleAlert size={13} /> {doomed.length} item stack
                  {doomed.length === 1 ? "" : "s"} will be deleted
                </div>
                <div className="st-doomed__list">
                  {doomed.map((s) => (
                    <div key={s.slot_index} className="st-doomed__row">
                      <ItemIcon staticId={s.static_id} size={22} />
                      <span className="st-doomed__name">{itemName(s.static_id)}</span>
                      <span className="st-doomed__count">×{s.count}</span>
                    </div>
                  ))}
                </div>
                <label className="st-ack">
                  <input type="checkbox" checked={ack} onChange={(e) => setAck(e.target.checked)} />
                  Delete these items
                </label>
              </div>
            )}

            <div className="modal__actions">
              <Button variant="ghost" onClick={onClose}>
                Cancel
              </Button>
              <Button
                variant={doomed.length ? "danger" : "primary"}
                onClick={confirm}
                disabled={!canConfirm}
                loading={resize.isPending}
              >
                Resize
              </Button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
