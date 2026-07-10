import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Locate, Minus, Plus } from "lucide-react";
import type { CSSProperties } from "react";
import type { Actor } from "../types/api";
import { worldToUv } from "../lib/mapProject";

// Served from public/ at the web root (works in dev and in the Tauri build).
const mapUrl = "/palworld-map.jpg";

type Vars = CSSProperties & Record<string, string | number>;

const COLOR: Record<string, string> = {
  Player: "var(--accent)",
  WildPal: "#aab2c0",
  BaseCampPal: "var(--good)",
  OtomoPal: "var(--accent-2)",
  NPC: "var(--warn)",
};

const MIN_SCALE = 1;
const MAX_SCALE = 9;

interface Transform {
  scale: number;
  tx: number;
  ty: number;
}

interface Props {
  actors: Actor[];
  visible: Set<string>;
  onHover: (a: Actor | null) => void;
}

function clamp(n: number, lo: number, hi: number) {
  return Math.min(hi, Math.max(lo, n));
}

export function WorldMapView({ actors, visible, onHover }: Props) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const pan = useRef<{ px: number; py: number; tx: number; ty: number } | null>(null);
  const [t, setT] = useState<Transform>({ scale: 1, tx: 0, ty: 0 });
  const [loaded, setLoaded] = useState(false);

  const width = () => viewportRef.current?.clientWidth ?? 0;

  const clampT = useCallback((next: Transform): Transform => {
    const w = width();
    const min = w * (1 - next.scale);
    return { scale: next.scale, tx: clamp(next.tx, min, 0), ty: clamp(next.ty, min, 0) };
  }, []);

  const zoomAt = useCallback(
    (factor: number, cx: number, cy: number) => {
      setT((prev) => {
        const scale = clamp(prev.scale * factor, MIN_SCALE, MAX_SCALE);
        if (scale === prev.scale) return prev;
        const sx = (cx - prev.tx) / prev.scale;
        const sy = (cy - prev.ty) / prev.scale;
        return clampT({ scale, tx: cx - scale * sx, ty: cy - scale * sy });
      });
    },
    [clampT],
  );

  // Native non-passive wheel so we can prevent page scroll.
  useEffect(() => {
    const el = viewportRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const rect = el.getBoundingClientRect();
      zoomAt(e.deltaY < 0 ? 1.2 : 1 / 1.2, e.clientX - rect.left, e.clientY - rect.top);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [zoomAt]);

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    pan.current = { px: e.clientX, py: e.clientY, tx: t.tx, ty: t.ty };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const p = pan.current;
    if (!p) return;
    setT((prev) => clampT({ scale: prev.scale, tx: p.tx + (e.clientX - p.px), ty: p.ty + (e.clientY - p.py) }));
  };
  const endPan = () => {
    pan.current = null;
  };

  const zoomButton = (factor: number) => {
    const w = width();
    zoomAt(factor, w / 2, w / 2);
  };
  const reset = () => setT({ scale: 1, tx: 0, ty: 0 });

  // Markers only depend on the data + filters, not the transform → stable across pan/zoom.
  const markers = useMemo(
    () =>
      actors
        .filter((a) => visible.has(a.UnitType))
        .map((a, i) => {
          const { u, v } = worldToUv(a.LocationX, a.LocationY);
          const isPlayer = a.UnitType === "Player";
          return (
            <button
              key={a.InstanceID || `${a.UnitType}-${i}`}
              className={`wm-mk${isPlayer ? " wm-mk--player" : ""}`}
              style={{ left: `${u * 100}%`, top: `${v * 100}%`, "--c": COLOR[a.UnitType] ?? "var(--dim)" } as Vars}
              onMouseEnter={() => onHover(a)}
              onMouseLeave={() => onHover(null)}
              aria-label={a.NickName || a.UnitType}
            />
          );
        }),
    [actors, visible, onHover],
  );

  return (
    <div
      ref={viewportRef}
      className="wm-view"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endPan}
      onPointerCancel={endPan}
    >
      <div
        className="wm-stage"
        style={{ transform: `translate(${t.tx}px, ${t.ty}px) scale(${t.scale})`, "--inv": 1 / t.scale } as Vars}
      >
        <img
          className="wm-map"
          src={mapUrl}
          alt="Palworld world map"
          draggable={false}
          onLoad={() => setLoaded(true)}
        />
        {loaded && markers}
      </div>

      {!loaded && <div className="wm-loading skeleton" />}

      <div className="wm-zoom">
        <button className="icobtn" onClick={() => zoomButton(1.5)} aria-label="Zoom in" title="Zoom in">
          <Plus size={15} />
        </button>
        <button className="icobtn" onClick={() => zoomButton(1 / 1.5)} aria-label="Zoom out" title="Zoom out">
          <Minus size={15} />
        </button>
        <button className="icobtn" onClick={reset} aria-label="Reset view" title="Reset view">
          <Locate size={15} />
        </button>
      </div>
    </div>
  );
}
