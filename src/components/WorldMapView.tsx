import { useCallback, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Locate, Minus, Plus } from "lucide-react";
import type { CSSProperties } from "react";
import type { Actor } from "../types/api";
import { worldToUv } from "../lib/mapProject";

// Served from public/ at the web root (works in dev and in the Tauri build).
const mapUrl = "/palworld-map.jpg";
// Native pixel size of the map image — the map layer is rendered at this size and
// scaled to fit, so zooming samples the full-resolution image (crisp, not upscaled).
const MAP_PX = 4096;

type Vars = CSSProperties & Record<string, string | number>;

const COLOR: Record<string, string> = {
  Player: "var(--accent)",
  WildPal: "#aab2c0",
  BaseCampPal: "var(--good)",
  OtomoPal: "var(--accent-2)",
  NPC: "var(--warn)",
};

interface T {
  s: number; // absolute scale: on-screen px = MAP_PX * s
  tx: number;
  ty: number;
}

interface Props {
  actors: Actor[];
  visible: Set<string>;
  onHover: (a: Actor | null) => void;
}

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

export function WorldMapView({ actors, visible, onHover }: Props) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const pan = useRef<{ px: number; py: number; tx: number; ty: number } | null>(null);
  const fitRef = useRef(0);
  const wRef = useRef(0);
  const [t, setT] = useState<T | null>(null);
  const [loaded, setLoaded] = useState(false);

  // Cap zoom at ~native resolution (1.15× the 4096px source) so the map never
  // upscales into blur — you can zoom right up to the map's real pixels, no further.
  const maxScale = () => Math.max(1.15, fitRef.current);

  const clampT = useCallback((v: T): T => {
    const fit = fitRef.current;
    const s = clamp(v.s, fit, maxScale());
    const disp = MAP_PX * s;
    const minOff = Math.min(0, wRef.current - disp);
    return { s, tx: clamp(v.tx, minOff, 0), ty: clamp(v.ty, minOff, 0) };
  }, []);

  const measure = useCallback(() => {
    const el = viewportRef.current;
    if (!el) return 0;
    const w = el.clientWidth;
    wRef.current = w;
    fitRef.current = w / MAP_PX;
    return w;
  }, []);

  useLayoutEffect(() => {
    if (measure() > 0) setT({ s: fitRef.current, tx: 0, ty: 0 });
    const ro = new ResizeObserver(() => {
      measure();
      setT((prev) => (prev ? clampT({ ...prev, s: Math.max(prev.s, fitRef.current) }) : { s: fitRef.current, tx: 0, ty: 0 }));
    });
    if (viewportRef.current) ro.observe(viewportRef.current);
    return () => ro.disconnect();
  }, [measure, clampT]);

  const zoomAt = useCallback(
    (factor: number, cx: number, cy: number) => {
      setT((prev) => {
        if (!prev) return prev;
        const s = clamp(prev.s * factor, fitRef.current, maxScale());
        if (s === prev.s) return prev;
        const px = (cx - prev.tx) / prev.s;
        const py = (cy - prev.ty) / prev.s;
        return clampT({ s, tx: cx - s * px, ty: cy - s * py });
      });
    },
    [clampT],
  );

  useLayoutEffect(() => {
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
    if (e.button !== 0 || !t) return;
    pan.current = { px: e.clientX, py: e.clientY, tx: t.tx, ty: t.ty };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const p = pan.current;
    if (!p) return;
    setT((prev) => (prev ? clampT({ ...prev, tx: p.tx + (e.clientX - p.px), ty: p.ty + (e.clientY - p.py) }) : prev));
  };
  const endPan = () => {
    pan.current = null;
  };

  const zoomButton = (factor: number) => zoomAt(factor, wRef.current / 2, wRef.current / 2);
  const reset = () => setT({ s: fitRef.current, tx: 0, ty: 0 });

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
      {t && (
        <div
          className="wm-stage"
          style={{
            width: MAP_PX,
            height: MAP_PX,
            transform: `translate(${t.tx}px, ${t.ty}px) scale(${t.s})`,
            "--inv": 1 / t.s,
          } as Vars}
        >
          <img className="wm-map" src={mapUrl} alt="Palworld world map" draggable={false} onLoad={() => setLoaded(true)} />
          {loaded && markers}
        </div>
      )}

      {!loaded && <div className="wm-loading skeleton" />}

      <div className="wm-zoom">
        <button className="icobtn" onClick={() => zoomButton(1.4)} aria-label="Zoom in" title="Zoom in">
          <Plus size={15} />
        </button>
        <button className="icobtn" onClick={() => zoomButton(1 / 1.4)} aria-label="Zoom out" title="Zoom out">
          <Minus size={15} />
        </button>
        <button className="icobtn" onClick={reset} aria-label="Reset view" title="Reset view">
          <Locate size={15} />
        </button>
      </div>
    </div>
  );
}
