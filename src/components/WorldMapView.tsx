import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, Layers, Locate, Maximize2, Minimize2, Minus, Plus, TriangleAlert } from "lucide-react";
import type { Actor } from "../types/api";
import { worldToGameCoords } from "../lib/mapProject";
import {
  actorToMarker,
  CONTENT,
  KIND_META,
  LANDMARK_MARKERS,
  MARKER_ORDER,
} from "../lib/mapData";
import type { MapMarker, MarkerKind } from "../lib/mapData";

const mapUrl = "/palworld-map.webp";
const MAP_PX = 8192;
const MAX_SCALE = 2;
const WHEEL_K = 0.0016;
const DBL_ZOOM = 1.9;
const KEY_PAN = 72;
const TAU = Math.PI * 2;

const Z: Record<MarkerKind, number> = {
  effigy: 0,
  fasttravel: 1,
  dungeon: 1,
  npc: 2,
  wildpal: 2,
  basepal: 3,
  boss: 3,
  otomopal: 4,
  player: 5,
};

interface Props {
  actors: Actor[];
  /** Keys (userId + lowercased names) of currently-connected players. */
  onlineKeys: Set<string>;
  fallback: boolean;
}

function drawPalCircle(
  ctx: CanvasRenderingContext2D,
  img: HTMLImageElement,
  x: number,
  y: number,
  r: number,
  ring: string,
  hover: boolean,
) {
  ctx.save();
  if (hover) {
    ctx.shadowColor = ring;
    ctx.shadowBlur = 10;
  }
  ctx.beginPath();
  ctx.arc(x, y, r, 0, TAU);
  ctx.fillStyle = "#0b0b0d";
  ctx.fill();
  ctx.restore();
  ctx.save();
  ctx.beginPath();
  ctx.arc(x, y, r - 1.5, 0, TAU);
  ctx.clip();
  ctx.drawImage(img, x - r, y - r, r * 2, r * 2);
  ctx.restore();
  ctx.beginPath();
  ctx.arc(x, y, r - 0.75, 0, TAU);
  ctx.strokeStyle = ring;
  ctx.lineWidth = hover ? 2.4 : 1.8;
  ctx.stroke();
}

function hpColor(pct: number) {
  return pct > 0.5 ? "#3ad19a" : pct > 0.25 ? "#e6b450" : "#ec6a6a";
}

// Player marker: the player figure on a dark disc, wrapped in an HP ring that
// fills clockwise from 12 o'clock in proportion to HP/MaxHP (green→amber→red).
// pct === null means no HP data (players from /players fallback) → a plain ring.
function drawPlayer(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  pct: number | null,
  color: string,
  img: HTMLImageElement | undefined,
  hover: boolean,
) {
  const R = hover ? 11.5 : 9.5;
  const lw = hover ? 3 : 2.4;
  ctx.beginPath();
  ctx.arc(x, y, R + 1, 0, TAU);
  ctx.fillStyle = "rgba(8,10,14,0.82)";
  ctx.fill();
  if (img && img.complete && img.naturalWidth) {
    const sz = hover ? 17 : 14;
    ctx.drawImage(img, x - sz / 2, y - sz / 2, sz, sz);
  } else {
    ctx.beginPath();
    ctx.arc(x, y, hover ? 4.6 : 3.7, 0, TAU);
    ctx.fillStyle = color;
    ctx.fill();
  }
  if (pct === null) {
    ctx.beginPath();
    ctx.arc(x, y, R, 0, TAU);
    ctx.strokeStyle = hover ? "#ffffff" : color;
    ctx.lineWidth = lw;
    ctx.stroke();
    return;
  }
  ctx.beginPath();
  ctx.arc(x, y, R, 0, TAU);
  ctx.strokeStyle = "rgba(255,255,255,0.18)";
  ctx.lineWidth = lw;
  ctx.stroke();
  ctx.save();
  if (hover) {
    ctx.shadowColor = hpColor(pct);
    ctx.shadowBlur = 8;
  }
  ctx.beginPath();
  ctx.arc(x, y, R, -Math.PI / 2, -Math.PI / 2 + Math.max(pct, 0.0001) * TAU);
  ctx.strokeStyle = hpColor(pct);
  ctx.lineWidth = lw;
  ctx.lineCap = "round";
  ctx.stroke();
  ctx.restore();
}

function drawMarker(
  ctx: CanvasRenderingContext2D,
  m: MapMarker,
  x: number,
  y: number,
  icons: Record<string, HTMLImageElement>,
  palIcons: Record<string, HTMLImageElement>,
  hover: boolean,
) {
  const meta = KIND_META[m.kind];
  if (m.kind === "player") {
    const hp = m.actor?.HP;
    const mhp = m.actor?.MaxHP;
    const pct =
      typeof hp === "number" && typeof mhp === "number" && mhp > 0
        ? Math.max(0, Math.min(1, hp / mhp))
        : null;
    drawPlayer(ctx, x, y, pct, meta.color, icons[m.kind], hover);
    return;
  }
  // Live Pals + boss Pals draw their real Pal icon in a colored ring
  // (gold = alpha, red = predator, otherwise the layer color).
  if (m.palKey) {
    const pimg = palIcons[m.palKey];
    if (pimg && pimg.complete && pimg.naturalWidth) {
      const isBoss = m.kind === "boss";
      const ring = isBoss ? (m.sub === "Predator" ? "#ec6a6a" : "#e6b450") : meta.color;
      const r = isBoss ? (hover ? 15 : 13) : hover ? 11 : 9;
      drawPalCircle(ctx, pimg, x, y, r, ring, hover);
      return;
    }
  }
  const img = meta.icon ? icons[m.kind] : undefined;
  if (img && img.complete && img.naturalWidth) {
    const sz = m.kind === "effigy" ? 16 : m.kind === "boss" ? 22 : 20;
    ctx.beginPath();
    ctx.arc(x, y, sz * 0.6, 0, TAU);
    ctx.fillStyle = "rgba(6,6,9,0.5)";
    ctx.fill();
    if (hover) {
      ctx.save();
      ctx.shadowColor = "#ffffff";
      ctx.shadowBlur = 9;
      ctx.drawImage(img, x - sz / 2, y - sz / 2, sz, sz);
      ctx.restore();
    } else {
      ctx.drawImage(img, x - sz / 2, y - sz / 2, sz, sz);
    }
    return;
  }
  const r = m.kind === "wildpal" ? 3.6 : 4.6;
  ctx.beginPath();
  ctx.arc(x, y, r + 1.4, 0, TAU);
  ctx.fillStyle = "rgba(0,0,0,0.65)";
  ctx.fill();
  ctx.beginPath();
  ctx.arc(x, y, r, 0, TAU);
  ctx.fillStyle = meta.color;
  ctx.fill();
  if (hover) {
    ctx.beginPath();
    ctx.arc(x, y, r + 3, 0, TAU);
    ctx.strokeStyle = "#ffffff";
    ctx.lineWidth = 2;
    ctx.stroke();
  }
}

export function WorldMapView({ actors, onlineKeys, fallback }: Props) {
  const canvasRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<HTMLDivElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const canvasElRef = useRef<HTMLCanvasElement>(null);
  const badgeRef = useRef<HTMLSpanElement>(null);

  const tf = useRef({ s: 0, tx: 0, ty: 0 });
  const fit = useRef(0); // whole-map fit (zoom-out floor)
  const size = useRef({ w: 0, h: 0 });
  const dprRef = useRef(1);
  const pointers = useRef(new Map<number, { x: number; y: number }>());
  const pan = useRef<{ x: number; y: number; tx: number; ty: number; t: number; vx: number; vy: number; moved: number } | null>(null);
  const pinch = useRef<{ dist: number; s: number } | null>(null);
  const anim = useRef<number | null>(null);
  const glide = useRef<number | null>(null);

  const markersRef = useRef<MapMarker[]>([]);
  const visibleRef = useRef<Set<MarkerKind>>(new Set());
  const showOfflineRef = useRef(false);
  const hoveredRef = useRef<MapMarker | null>(null);
  const iconsRef = useRef<Record<string, HTMLImageElement>>({});
  const palIconsRef = useRef<Record<string, HTMLImageElement>>({});
  const palRaf = useRef(0);

  const [loaded, setLoaded] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [panelOpen, setPanelOpen] = useState(() => {
    try {
      return localStorage.getItem("psm.map.panel") !== "0";
    } catch {
      return true;
    }
  });
  const [visible, setVisible] = useState<Set<MarkerKind>>(() => {
    try {
      const raw = localStorage.getItem("psm.map.layers");
      if (raw) {
        const arr = (JSON.parse(raw) as string[]).filter((k): k is MarkerKind =>
          (MARKER_ORDER as string[]).includes(k),
        );
        return new Set(arr);
      }
    } catch {
      /* ignore */
    }
    return new Set(MARKER_ORDER.filter((k) => KIND_META[k].on));
  });
  const [showOffline, setShowOffline] = useState(() => {
    try {
      return localStorage.getItem("psm.map.offline") === "1";
    } catch {
      return false;
    }
  });
  const [hovered, setHovered] = useState<MapMarker | null>(null);

  const allMarkers = useMemo(() => {
    const live = actors.map((a, i) => actorToMarker(a, i, onlineKeys));
    const merged = [...live, ...LANDMARK_MARKERS];
    merged.sort((a, b) => Z[a.kind] - Z[b.kind]);
    return merged;
  }, [actors, onlineKeys]);

  const counts = useMemo(() => {
    const c = {} as Record<MarkerKind, number>;
    for (const k of MARKER_ORDER) c[k] = 0;
    for (const m of allMarkers) {
      if (m.kind === "player" && !showOffline && m.online === false) continue;
      c[m.kind]++;
    }
    return c;
  }, [allMarkers, showOffline]);

  const draw = useCallback(() => {
    const cv = canvasElRef.current;
    const ctx = cv?.getContext("2d");
    if (!cv || !ctx) return;
    const { w, h } = size.current;
    ctx.setTransform(dprRef.current, 0, 0, dprRef.current, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const { s, tx, ty } = tf.current;
    const span = MAP_PX * s;
    const vis = visibleRef.current;
    const showOff = showOfflineRef.current;
    const icons = iconsRef.current;
    const palIcons = palIconsRef.current;
    const hoverId = hoveredRef.current?.id;
    let hov: { m: MapMarker; x: number; y: number } | null = null;
    for (const m of markersRef.current) {
      if (!vis.has(m.kind)) continue;
      if (m.kind === "player" && !showOff && m.online === false) continue;
      const x = tx + m.u * span;
      const y = ty + m.v * span;
      if (x < -24 || y < -24 || x > w + 24 || y > h + 24) continue;
      drawMarker(ctx, m, x, y, icons, palIcons, false);
      if (m.id === hoverId) hov = { m, x, y };
    }
    if (hov) drawMarker(ctx, hov.m, hov.x, hov.y, icons, palIcons, true);
  }, []);

  const apply = useCallback(() => {
    const st = stageRef.current;
    if (st) {
      const { s, tx, ty } = tf.current;
      st.style.transform = `translate(${tx}px, ${ty}px) scale(${s})`;
      if (badgeRef.current && fit.current > 0) badgeRef.current.textContent = `${(s / fit.current).toFixed(1)}×`;
    }
    draw();
  }, [draw]);

  const clamp = useCallback(() => {
    const { w, h } = size.current;
    const s = Math.min(MAX_SCALE, Math.max(fit.current, tf.current.s));
    const disp = MAP_PX * s;
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
      const tick = (now: number) => {
        const k = Math.min(1, (now - t0) / 200);
        const e = 1 - Math.pow(1 - k, 3);
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

  // Default/reset view: fill the viewport with the island content (not empty ocean).
  const fitView = useCallback(() => {
    stopAnim();
    stopGlide();
    const { w, h } = size.current;
    if (w === 0 || h === 0) return;
    const cw = (CONTENT.uMax - CONTENT.uMin) * MAP_PX;
    const ch = (CONTENT.vMax - CONTENT.vMin) * MAP_PX;
    const s = Math.min(MAX_SCALE, Math.max(fit.current, Math.min(w / cw, h / ch)));
    const cu = (CONTENT.uMin + CONTENT.uMax) / 2;
    const cv = (CONTENT.vMin + CONTENT.vMax) / 2;
    tf.current = { s, tx: w / 2 - cu * MAP_PX * s, ty: h / 2 - cv * MAP_PX * s };
    clamp();
    apply();
  }, [clamp, apply]);

  const measure = useCallback(() => {
    const el = viewRef.current;
    if (!el) return;
    const w = el.clientWidth;
    const h = el.clientHeight;
    size.current = { w, h };
    fit.current = Math.min(w, h) / MAP_PX;
    const cv = canvasElRef.current;
    if (cv) {
      const dpr = Math.min(2, window.devicePixelRatio || 1);
      dprRef.current = dpr;
      cv.width = Math.round(w * dpr);
      cv.height = Math.round(h * dpr);
      cv.style.width = `${w}px`;
      cv.style.height = `${h}px`;
    }
  }, []);

  useLayoutEffect(() => {
    measure();
    fitView();
    const ro = new ResizeObserver(() => {
      measure();
      clamp();
      apply();
    });
    if (viewRef.current) ro.observe(viewRef.current);
    return () => ro.disconnect();
  }, [measure, fitView, clamp, apply]);

  useLayoutEffect(() => {
    measure();
    fitView();
  }, [expanded, measure, fitView]);

  // Preload landmark icons.
  useEffect(() => {
    const map: Record<string, HTMLImageElement> = {};
    for (const k of MARKER_ORDER) {
      const icon = KIND_META[k].icon;
      if (!icon) continue;
      const img = new Image();
      img.onload = () => draw();
      img.src = icon;
      map[k] = img;
    }
    iconsRef.current = map;
  }, [draw]);

  // Load Pal icons on demand for every species present (boss + live Pals).
  useEffect(() => {
    const cache = palIconsRef.current;
    const redraw = () => {
      if (!palRaf.current)
        palRaf.current = requestAnimationFrame(() => {
          palRaf.current = 0;
          draw();
        });
    };
    let added = false;
    for (const m of allMarkers) {
      const key = m.palKey;
      if (key && !cache[key]) {
        const img = new Image();
        img.onload = redraw;
        img.src = `/mapicons/pals/${key}.webp`;
        cache[key] = img;
        added = true;
      }
    }
    if (added) draw();
  }, [allMarkers, draw]);

  useEffect(() => {
    markersRef.current = allMarkers;
    draw();
  }, [allMarkers, draw]);
  useEffect(() => {
    visibleRef.current = visible;
    showOfflineRef.current = showOffline;
    draw();
    try {
      localStorage.setItem("psm.map.layers", JSON.stringify([...visible]));
      localStorage.setItem("psm.map.offline", showOffline ? "1" : "0");
    } catch {
      /* ignore */
    }
  }, [visible, showOffline, draw]);
  useEffect(() => {
    hoveredRef.current = hovered;
    draw();
  }, [hovered, draw]);
  useEffect(() => {
    try {
      localStorage.setItem("psm.map.panel", panelOpen ? "1" : "0");
    } catch {
      /* ignore */
    }
  }, [panelOpen]);

  useEffect(() => {
    const el = viewRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if ((e.target as Element).closest?.(".wm-nozoom")) return;
      e.preventDefault();
      stopAnim();
      stopGlide();
      const r = el.getBoundingClientRect();
      zoomAt(tf.current.s * Math.exp(-e.deltaY * WHEEL_K), e.clientX - r.left, e.clientY - r.top);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [zoomAt]);

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

  const hitTest = (mx: number, my: number): MapMarker | null => {
    const { s, tx, ty } = tf.current;
    const span = MAP_PX * s;
    const vis = visibleRef.current;
    const showOff = showOfflineRef.current;
    let best: MapMarker | null = null;
    let bestD = 14 * 14;
    for (const m of markersRef.current) {
      if (!vis.has(m.kind)) continue;
      if (m.kind === "player" && !showOff && m.online === false) continue;
      const x = tx + m.u * span - mx;
      const y = ty + m.v * span - my;
      const d = x * x + y * y;
      if (d <= bestD) {
        bestD = d;
        best = m;
      }
    }
    return best;
  };

  const setHoverIfChanged = (m: MapMarker | null) => {
    if ((m?.id ?? null) !== (hoveredRef.current?.id ?? null)) setHovered(m);
  };

  const onPointerDown = (e: React.PointerEvent) => {
    if ((e.target as Element).closest?.(".wm-nozoom")) return;
    if (e.pointerType === "mouse" && e.button !== 0) return;
    stopAnim();
    stopGlide();
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      /* ignore */
    }
    pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.current.size === 2) {
      pan.current = null;
      const [a, b] = [...pointers.current.values()];
      pinch.current = { dist: Math.hypot(a.x - b.x, a.y - b.y) || 1, s: tf.current.s };
    } else {
      pan.current = { x: e.clientX, y: e.clientY, tx: tf.current.tx, ty: tf.current.ty, t: performance.now(), vx: 0, vy: 0, moved: 0 };
    }
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!pointers.current.has(e.pointerId)) {
      if (e.pointerType === "mouse") {
        const r = viewRef.current!.getBoundingClientRect();
        setHoverIfChanged(hitTest(e.clientX - r.left, e.clientY - r.top));
      }
      return;
    }
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
      pan.current.moved += Math.abs(ntx - tf.current.tx) + Math.abs(nty - tf.current.ty);
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
    const wasTap = e.pointerType !== "mouse" && pan.current !== null && pan.current.moved < 6;
    pointers.current.delete(e.pointerId);
    if (pointers.current.size < 2) pinch.current = null;
    if (pointers.current.size === 0) {
      if (wasTap && viewRef.current) {
        const r = viewRef.current.getBoundingClientRect();
        setHoverIfChanged(hitTest(e.clientX - r.left, e.clientY - r.top));
      } else if (pan.current) {
        startGlide(pan.current.vx, pan.current.vy);
      }
      pan.current = null;
    } else if (pointers.current.size === 1) {
      const [p] = [...pointers.current.values()];
      pan.current = { x: p.x, y: p.y, tx: tf.current.tx, ty: tf.current.ty, t: performance.now(), vx: 0, vy: 0, moved: 0 };
    }
  };

  const onPointerLeave = () => {
    if (pointers.current.size === 0) setHoverIfChanged(null);
  };

  const onDoubleClick = (e: React.MouseEvent) => {
    if ((e.target as Element).closest?.(".wm-nozoom")) return;
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
        fitView();
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

  const toggleExpand = () => setExpanded((v) => !v);
  useEffect(() => {
    if (!expanded) return;
    const el = canvasRef.current;
    if (el && !document.fullscreenElement && el.requestFullscreen) el.requestFullscreen().catch(() => {});
    const onFsChange = () => {
      if (!document.fullscreenElement) setExpanded(false);
    };
    document.addEventListener("fullscreenchange", onFsChange);
    return () => {
      document.removeEventListener("fullscreenchange", onFsChange);
      if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
    };
  }, [expanded]);

  const toggleLayer = (k: MarkerKind) =>
    setVisible((prev) => {
      const next = new Set(prev);
      if (next.has(k)) next.delete(k);
      else next.add(k);
      return next;
    });

  const stop = (e: React.SyntheticEvent) => e.stopPropagation();
  const gc = hovered ? worldToGameCoords(hovered.x, hovered.y) : null;
  const liveKinds = MARKER_ORDER.filter((k) => KIND_META[k].group === "live");
  const landmarkKinds = MARKER_ORDER.filter((k) => KIND_META[k].group === "landmark");

  const layerRow = (k: MarkerKind) => {
    const meta = KIND_META[k];
    const on = visible.has(k);
    return (
      <button key={k} className={`wm-layer${on ? "" : " is-off"}`} onClick={() => toggleLayer(k)} aria-pressed={on}>
        {meta.icon ? (
          <img className="wm-layer__ic" src={meta.icon} alt="" />
        ) : (
          <span className="wm-layer__dot" style={{ background: meta.color }} />
        )}
        <span className="wm-layer__label">{meta.label}</span>
        <span className="wm-layer__count">{counts[k]}</span>
      </button>
    );
  };

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
        onPointerLeave={onPointerLeave}
        onDoubleClick={onDoubleClick}
        onKeyDown={onKeyDown}
      >
        <div className="wm-stage" ref={stageRef} style={{ width: MAP_PX, height: MAP_PX }}>
          <img className="wm-map" src={mapUrl} alt="Palworld world map" draggable={false} onLoad={() => setLoaded(true)} />
        </div>
        <canvas className="wm-markers" ref={canvasElRef} />

        {!loaded && <div className="wm-loading skeleton" />}

        <div className="wm-panel wm-layers wm-nozoom" onPointerDown={stop} onDoubleClick={stop}>
          <button className="wm-panel__head" onClick={() => setPanelOpen((o) => !o)}>
            <Layers size={14} />
            <span>Layers</span>
            <ChevronDown size={14} className={`wm-panel__chev${panelOpen ? " is-open" : ""}`} />
          </button>
          {panelOpen && (
            <div className="wm-panel__body">
              <div className="wm-lgroup">Live</div>
              {liveKinds.map(layerRow)}
              <label className="wm-offline">
                <input type="checkbox" checked={showOffline} onChange={(e) => setShowOffline(e.target.checked)} />
                Show offline players
              </label>
              <div className="wm-lgroup">Landmarks</div>
              {landmarkKinds.map(layerRow)}
            </div>
          )}
        </div>

        {hovered && gc && (
          <div className="wm-panel wm-detail wm-nozoom" onPointerDown={stop} onDoubleClick={stop}>
            <div className="wm-detail__top">
              <span className="wm-detail__dot" style={{ background: KIND_META[hovered.kind].color }} />
              <b>{hovered.name}</b>
              <span className="wm-detail__kind">{KIND_META[hovered.kind].label.replace(/s$/, "")}</span>
            </div>
            <div className="wm-detail__rows">
              {!hovered.actor && hovered.sub && (
                <div>
                  <span>Type</span>
                  <span>{hovered.sub}</span>
                </div>
              )}
              {hovered.actor?.level != null && (
                <div>
                  <span>Level</span>
                  <span>{hovered.actor.level}</span>
                </div>
              )}
              {hovered.actor?.HP != null && hovered.actor?.MaxHP != null && (
                <div>
                  <span>HP</span>
                  <span>
                    {hovered.actor.HP} / {hovered.actor.MaxHP}
                    {hovered.actor.MaxHP > 0 &&
                      ` (${Math.round((hovered.actor.HP / hovered.actor.MaxHP) * 100)}%)`}
                  </span>
                </div>
              )}
              {hovered.actor?.GuildName && (
                <div>
                  <span>Guild</span>
                  <span>{hovered.actor.GuildName}</span>
                </div>
              )}
              <div>
                <span>Coords</span>
                <span className="mono">
                  {gc.x}, {gc.y}
                </span>
              </div>
              {hovered.actor?.Class && (
                <div>
                  <span>Class</span>
                  <span
                    className="mono"
                    title={hovered.actor.Class}
                    style={{ maxWidth: 150, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
                  >
                    {hovered.actor.Class}
                  </span>
                </div>
              )}
            </div>
          </div>
        )}

        {fallback && (
          <div className="wm-note wm-nozoom">
            <TriangleAlert size={13} />
            Players only — enable the GameData API to see Pals.
          </div>
        )}

        <div className="wm-ctrls wm-nozoom" onPointerDown={stop} onDoubleClick={stop}>
          <button className="wm-ctrl" onClick={() => zoomButton(1.4)} aria-label="Zoom in" title="Zoom in">
            <Plus size={16} />
          </button>
          <button className="wm-ctrl" onClick={() => zoomButton(1 / 1.4)} aria-label="Zoom out" title="Zoom out">
            <Minus size={16} />
          </button>
          <button className="wm-ctrl" onClick={fitView} aria-label="Fit map" title="Fit map">
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

        <div className="wm-badge wm-nozoom">
          <span ref={badgeRef}>1.0×</span>
        </div>
      </div>
    </div>
  );
}
