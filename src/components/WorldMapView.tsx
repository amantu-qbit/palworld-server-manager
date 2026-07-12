import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Locate, Maximize2, Minimize2, Minus, Plus } from "lucide-react";
import type { CSSProperties } from "react";
import type { Actor } from "../types/api";
import { worldToGameCoords, worldToUv } from "../lib/mapProject";

// Served from public/ at the web root (works in dev and in the Tauri build).
const mapUrl = "/palworld-map.webp";
// Native pixel size of the map texture (public/palworld-map.webp is 8192²). The stage
// is laid out at this size and transformed; markers position by % so they're unaffected.
const MAP_PX = 8192;
// Deepest zoom: 2× the native texture — plenty of inspection room, still legible.
const MAX_SCALE = 2;
const WHEEL_K = 0.0016; // wheel/trackpad zoom sensitivity
const DBL_ZOOM = 1.9; // double-click zoom-in factor
const KEY_PAN = 72; // px per arrow-key press

type Vars = CSSProperties & Record<string, string | number>;

const COLOR: Record<string, string> = {
  Player: "var(--accent)",
  WildPal: "#aab2c0",
  BaseCampPal: "var(--good)",
  OtomoPal: "var(--accent-2)",
  NPC: "var(--warn)",
};

interface Transform {
  s: number;
  tx: number;
  ty: number;
}

interface Props {
  actors: Actor[];
  visible: Set<string>;
  onHover: (a: Actor | null) => void;
  /** Currently hovered actor (from the parent), shown in the fullscreen readout. */
  hovered: Actor | null;
}

export function WorldMapView({ actors, visible, onHover, hovered }: Props) {
  const canvasRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<HTMLDivElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const badgeRef = useRef<HTMLSpanElement>(null);

  // Live transform + gesture bookkeeping live in refs so pan/zoom never re-render React.
  const tf = useRef<Transform>({ s: 0, tx: 0, ty: 0 });
  const fit = useRef(0);
  const size = useRef({ w: 0, h: 0 });
  const pointers = useRef(new Map<number, { x: number; y: number }>());
  const pan = useRef<{ x: number; y: number; tx: number; ty: number; t: number; vx: number; vy: number } | null>(null);
  const pinch = useRef<{ dist: number; s: number } | null>(null);
  const anim = useRef<number | null>(null);
  const glide = useRef<number | null>(null);

  const [loaded, setLoaded] = useState(false);
  const [expanded, setExpanded] = useState(false);

  // ---- imperative transform ----
  const apply = useCallback(() => {
    const st = stageRef.current;
    if (!st) return;
    const { s, tx, ty } = tf.current;
    st.style.transform = `translate(${tx}px, ${ty}px) scale(${s})`;
    st.style.setProperty("--inv", String(1 / s));
    if (badgeRef.current && fit.current > 0) badgeRef.current.textContent = `${(s / fit.current).toFixed(1)}×`;
  }, []);

  const clamp = useCallback(() => {
    const { w, h } = size.current;
    const s = Math.min(MAX_SCALE, Math.max(fit.current, tf.current.s));
    const disp = MAP_PX * s;
    // Centre the axis when the map is smaller than the viewport; otherwise keep it
    // covering the viewport (can't drag the map off into empty space).
    const axis = (t: number, v: number) => (disp <= v ? (v - disp) / 2 : Math.min(0, Math.max(v - disp, t)));
    tf.current = { s, tx: axis(tf.current.tx, w), ty: axis(tf.current.ty, h) };
  }, []);

  const stopAnim = () => {
    if (anim.current !== null) cancelAnimationFrame(anim.current);
    anim.current = null;
  };
  const stopGlide = () => {
    if (glide.current !== null) cancelAnimationFrame(glide.current);
    glide.current = null;
  };

  // Zoom to `target` scale while keeping the point (cx,cy) — in viewport px — fixed.
  const zoomAt = useCallback(
    (target: number, cx: number, cy: number) => {
      const s0 = tf.current.s;
      const s1 = Math.min(MAX_SCALE, Math.max(fit.current, target));
      if (s1 === s0) return;
      const px = (cx - tf.current.tx) / s0;
      const py = (cy - tf.current.ty) / s0;
      tf.current.s = s1;
      tf.current.tx = cx - px * s1;
      tf.current.ty = cy - py * s1;
      clamp();
      apply();
    },
    [clamp, apply],
  );

  const animateZoom = useCallback(
    (target: number, cx: number, cy: number) => {
      stopAnim();
      stopGlide();
      const start = tf.current.s;
      const end = Math.min(MAX_SCALE, Math.max(fit.current, target));
      if (Math.abs(end - start) < 1e-4) return;
      const t0 = performance.now();
      const dur = 200;
      const tick = (now: number) => {
        const k = Math.min(1, (now - t0) / dur);
        const e = 1 - Math.pow(1 - k, 3); // easeOutCubic
        zoomAt(start + (end - start) * e, cx, cy);
        anim.current = k < 1 ? requestAnimationFrame(tick) : null;
      };
      anim.current = requestAnimationFrame(tick);
    },
    [zoomAt],
  );

  const center = () => ({ cx: size.current.w / 2, cy: size.current.h / 2 });
  const zoomButton = (mult: number) => {
    const { cx, cy } = center();
    animateZoom(tf.current.s * mult, cx, cy);
  };

  const doFit = useCallback(() => {
    stopAnim();
    stopGlide();
    tf.current = { s: fit.current, tx: 0, ty: 0 };
    clamp();
    apply();
  }, [clamp, apply]);

  const measure = useCallback(() => {
    const el = viewRef.current;
    if (!el) return;
    size.current = { w: el.clientWidth, h: el.clientHeight };
    fit.current = Math.min(size.current.w, size.current.h) / MAP_PX;
  }, []);

  // Initial measure + keep fitting on resize (covers entering/leaving fullscreen).
  useLayoutEffect(() => {
    measure();
    if (tf.current.s <= 0) tf.current.s = fit.current;
    clamp();
    apply();
    const ro = new ResizeObserver(() => {
      measure();
      clamp();
      apply();
    });
    if (viewRef.current) ro.observe(viewRef.current);
    return () => ro.disconnect();
  }, [measure, clamp, apply]);

  // Re-fit to the viewport whenever we enter/leave the fullscreen overlay. Runs after
  // the class commit, and measure() forces a reflow so it reads the true new size.
  useLayoutEffect(() => {
    measure();
    tf.current = { s: fit.current, tx: 0, ty: 0 };
    clamp();
    apply();
  }, [expanded, measure, clamp, apply]);

  // Wheel / trackpad zoom (non-passive so we can preventDefault).
  useEffect(() => {
    const el = viewRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      stopAnim();
      stopGlide();
      const r = el.getBoundingClientRect();
      zoomAt(tf.current.s * Math.exp(-e.deltaY * WHEEL_K), e.clientX - r.left, e.clientY - r.top);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [zoomAt]);

  // ---- pan momentum ----
  const startGlide = (vx0: number, vy0: number) => {
    let vx = vx0 * 16;
    let vy = vy0 * 16;
    if (Math.hypot(vx, vy) < 0.6) return;
    const tick = () => {
      vx *= 0.92;
      vy *= 0.92;
      tf.current.tx += vx;
      tf.current.ty += vy;
      clamp();
      apply();
      glide.current = Math.hypot(vx, vy) > 0.4 ? requestAnimationFrame(tick) : null;
    };
    glide.current = requestAnimationFrame(tick);
  };

  // ---- pointer gestures (pan + pinch) ----
  const onPointerDown = (e: React.PointerEvent) => {
    if (e.pointerType === "mouse" && e.button !== 0) return;
    stopAnim();
    stopGlide();
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      /* synthetic events may not support capture */
    }
    pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.current.size === 2) {
      pan.current = null;
      const [a, b] = [...pointers.current.values()];
      pinch.current = { dist: Math.hypot(a.x - b.x, a.y - b.y) || 1, s: tf.current.s };
    } else {
      pan.current = { x: e.clientX, y: e.clientY, tx: tf.current.tx, ty: tf.current.ty, t: performance.now(), vx: 0, vy: 0 };
    }
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!pointers.current.has(e.pointerId)) return;
    pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
    const el = viewRef.current;
    if (pinch.current && pointers.current.size >= 2 && el) {
      const [a, b] = [...pointers.current.values()];
      const d = Math.hypot(a.x - b.x, a.y - b.y);
      const r = el.getBoundingClientRect();
      zoomAt(pinch.current.s * (d / pinch.current.dist), (a.x + b.x) / 2 - r.left, (a.y + b.y) / 2 - r.top);
    } else if (pan.current) {
      const now = performance.now();
      const ntx = pan.current.tx + (e.clientX - pan.current.x);
      const nty = pan.current.ty + (e.clientY - pan.current.y);
      const dt = now - pan.current.t || 16;
      pan.current.vx = (ntx - tf.current.tx) / dt;
      pan.current.vy = (nty - tf.current.ty) / dt;
      pan.current.t = now;
      tf.current.tx = ntx;
      tf.current.ty = nty;
      clamp();
      apply();
    }
  };

  const onPointerUp = (e: React.PointerEvent) => {
    if (!pointers.current.has(e.pointerId)) return;
    try {
      e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {
      /* ignore */
    }
    pointers.current.delete(e.pointerId);
    if (pointers.current.size < 2) pinch.current = null;
    if (pointers.current.size === 0) {
      if (pan.current) {
        startGlide(pan.current.vx, pan.current.vy);
        pan.current = null;
      }
    } else if (pointers.current.size === 1) {
      const [p] = [...pointers.current.values()];
      pan.current = { x: p.x, y: p.y, tx: tf.current.tx, ty: tf.current.ty, t: performance.now(), vx: 0, vy: 0 };
    }
  };

  const onDoubleClick = (e: React.MouseEvent) => {
    const el = viewRef.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    animateZoom(tf.current.s * DBL_ZOOM, e.clientX - r.left, e.clientY - r.top);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    let handled = true;
    switch (e.key) {
      case "ArrowLeft":
        tf.current.tx += KEY_PAN;
        clamp();
        apply();
        break;
      case "ArrowRight":
        tf.current.tx -= KEY_PAN;
        clamp();
        apply();
        break;
      case "ArrowUp":
        tf.current.ty += KEY_PAN;
        clamp();
        apply();
        break;
      case "ArrowDown":
        tf.current.ty -= KEY_PAN;
        clamp();
        apply();
        break;
      case "+":
      case "=":
        zoomButton(1.3);
        break;
      case "-":
      case "_":
        zoomButton(1 / 1.3);
        break;
      case "0":
        doFit();
        break;
      case "Escape":
        if (expanded) setExpanded(false);
        else handled = false;
        break;
      default:
        handled = false;
    }
    if (handled) e.preventDefault();
  };

  // ---- fullscreen ----
  const toggleExpand = () => setExpanded((v) => !v);

  useEffect(() => {
    if (!expanded) return;
    const el = canvasRef.current;
    if (el && !document.fullscreenElement && el.requestFullscreen) {
      el.requestFullscreen().catch(() => {
        /* fall back to the in-window overlay */
      });
    }
    const onFsChange = () => {
      if (!document.fullscreenElement) setExpanded(false);
    };
    document.addEventListener("fullscreenchange", onFsChange);
    return () => {
      document.removeEventListener("fullscreenchange", onFsChange);
      if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
    };
  }, [expanded]);

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

  const hoverCoords = hovered ? worldToGameCoords(hovered.LocationX, hovered.LocationY) : null;

  return (
    <div ref={canvasRef} className={`wm-canvas${expanded ? " wm-canvas--fs" : ""}`}>
      <div
        ref={viewRef}
        className="wm-view"
        tabIndex={0}
        role="application"
        aria-label="World map — drag to pan, scroll to zoom"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
        onDoubleClick={onDoubleClick}
        onKeyDown={onKeyDown}
      >
        <div className="wm-stage" ref={stageRef} style={{ width: MAP_PX, height: MAP_PX } as Vars}>
          <img className="wm-map" src={mapUrl} alt="Palworld world map" draggable={false} onLoad={() => setLoaded(true)} />
          {loaded && markers}
        </div>

        {!loaded && <div className="wm-loading skeleton" />}

        <div className="wm-ctrls" onPointerDown={(e) => e.stopPropagation()} onDoubleClick={(e) => e.stopPropagation()}>
          <button className="wm-ctrl" onClick={() => zoomButton(1.4)} aria-label="Zoom in" title="Zoom in">
            <Plus size={16} />
          </button>
          <button className="wm-ctrl" onClick={() => zoomButton(1 / 1.4)} aria-label="Zoom out" title="Zoom out">
            <Minus size={16} />
          </button>
          <button className="wm-ctrl" onClick={doFit} aria-label="Fit map" title="Fit map">
            <Locate size={16} />
          </button>
          <button
            className="wm-ctrl"
            onClick={toggleExpand}
            aria-label={expanded ? "Exit fullscreen" : "Fullscreen"}
            title={expanded ? "Exit fullscreen (Esc)" : "Fullscreen"}
          >
            {expanded ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
          </button>
        </div>

        <div className="wm-badge">
          <span ref={badgeRef}>1.0×</span>
        </div>

        {expanded && (
          <div className="wm-fsbar">
            {hovered && hoverCoords ? (
              <span className="wm-fsbar__sel">
                <b>{hovered.NickName || hovered.Class || hovered.UnitType}</b>
                <span className="wm-fsbar__coords">
                  {hoverCoords.x}, {hoverCoords.y}
                </span>
              </span>
            ) : (
              <span className="wm-fsbar__hint">
                Scroll to zoom · drag to pan · double-click to zoom · arrows to move · Esc to exit
              </span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
