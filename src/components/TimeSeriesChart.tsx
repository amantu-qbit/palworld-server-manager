import { useCallback, useEffect, useMemo, useRef, useState } from "react";

export interface ChartPoint {
  t: number;
  v: number;
}

interface Props {
  points: ChartPoint[];
  stroke?: string;
  height?: number;
  unit?: string;
  /** Fix the y-axis floor/ceiling; otherwise auto-scaled from the data. */
  yMin?: number;
  yMax?: number;
  /** Value formatter for gridline labels + the hover tooltip. */
  format?: (v: number) => string;
  /** Segments split when two consecutive samples are more than this apart (ms). */
  gapMs?: number;
}

const M = { top: 10, right: 14, bottom: 22, left: 46 };

/** Width of the element, tracked live so the SVG draws at true pixel size. */
function useWidth<T extends HTMLElement>() {
  const ref = useRef<T | null>(null);
  const [w, setW] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) setW(e.contentRect.width);
    });
    ro.observe(el);
    setW(el.clientWidth);
    return () => ro.disconnect();
  }, []);
  return [ref, w] as const;
}

const clockFmt = new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" });
const stampFmt = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
});

/**
 * A compact SVG time-series chart — the Sparkline's grown-up sibling: real time
 * X-axis, a few value gridlines, gap-aware line segments, and a hover readout.
 * No charting dependency; matches the hand-built, restrained house style.
 */
export function TimeSeriesChart({
  points,
  stroke = "var(--accent)",
  height = 200,
  unit = "",
  yMin,
  yMax,
  format = (v) => `${Math.round(v)}`,
  gapMs = 150_000,
}: Props) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const [hoverX, setHoverX] = useState<number | null>(null);

  const iw = Math.max(1, width - M.left - M.right);
  const ih = Math.max(1, height - M.top - M.bottom);

  const domain = useMemo(() => {
    if (points.length === 0) return { tMin: 0, tMax: 1, vMin: 0, vMax: 1 };
    let t0 = Infinity;
    let t1 = -Infinity;
    let v0 = Infinity;
    let v1 = -Infinity;
    for (const p of points) {
      if (p.t < t0) t0 = p.t;
      if (p.t > t1) t1 = p.t;
      if (p.v < v0) v0 = p.v;
      if (p.v > v1) v1 = p.v;
    }
    const lo = yMin ?? v0;
    const hi = yMax ?? v1;
    const pad = (hi - lo) * 0.12 || 1;
    return {
      tMin: t0,
      tMax: t1 === t0 ? t0 + 1 : t1,
      vMin: yMin ?? lo - pad,
      vMax: yMax ?? hi + pad,
    };
  }, [points, yMin, yMax]);

  const { tMin, tMax, vMin, vMax } = domain;
  const xOf = useCallback((t: number) => M.left + (iw * (t - tMin)) / (tMax - tMin), [iw, tMin, tMax]);
  const yOf = useCallback(
    (v: number) => M.top + ih * (1 - (v - vMin) / (vMax - vMin || 1)),
    [ih, vMin, vMax],
  );

  // Break the line wherever the app wasn't collecting (a gap > gapMs).
  const segments = useMemo(() => {
    const segs: ChartPoint[][] = [];
    let cur: ChartPoint[] = [];
    for (let i = 0; i < points.length; i++) {
      if (i > 0 && points[i].t - points[i - 1].t > gapMs) {
        if (cur.length) segs.push(cur);
        cur = [];
      }
      cur.push(points[i]);
    }
    if (cur.length) segs.push(cur);
    return segs;
  }, [points, gapMs]);

  const yTicks = useMemo(() => {
    const n = 4;
    return Array.from({ length: n + 1 }, (_, i) => vMin + ((vMax - vMin) * i) / n);
  }, [vMin, vMax]);

  const xTicks = useMemo(() => {
    const n = Math.min(4, Math.max(1, points.length - 1));
    return Array.from({ length: n + 1 }, (_, i) => tMin + ((tMax - tMin) * i) / n);
  }, [tMin, tMax, points.length]);

  const nearest = useMemo(() => {
    if (hoverX == null || points.length === 0) return null;
    let best = points[0];
    let bestD = Infinity;
    for (const p of points) {
      const d = Math.abs(xOf(p.t) - hoverX);
      if (d < bestD) {
        bestD = d;
        best = p;
      }
    }
    return best;
  }, [hoverX, points, xOf]);

  const gid = `ts-${stroke.replace(/[^a-z0-9]/gi, "")}`;

  if (points.length < 2) {
    return (
      <div className="ts-empty" ref={ref} style={{ height }}>
        Not enough data yet — trends fill in as the app keeps polling.
      </div>
    );
  }

  return (
    <div className="ts" ref={ref} style={{ height }}>
      <svg
        width="100%"
        height={height}
        role="img"
        onPointerMove={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          setHoverX(e.clientX - rect.left);
        }}
        onPointerLeave={() => setHoverX(null)}
      >
        <defs>
          <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor={stroke} stopOpacity="0.2" />
            <stop offset="1" stopColor={stroke} stopOpacity="0" />
          </linearGradient>
        </defs>

        {/* Y gridlines + labels */}
        {yTicks.map((v, i) => (
          <g key={i}>
            <line
              className="ts-grid"
              x1={M.left}
              x2={width - M.right}
              y1={yOf(v)}
              y2={yOf(v)}
            />
            <text className="ts-ylabel" x={M.left - 8} y={yOf(v) + 3} textAnchor="end">
              {format(v)}
            </text>
          </g>
        ))}

        {/* X time labels */}
        {xTicks.map((t, i) => (
          <text
            key={i}
            className="ts-xlabel"
            x={xOf(t)}
            y={height - 6}
            textAnchor={i === 0 ? "start" : i === xTicks.length - 1 ? "end" : "middle"}
          >
            {clockFmt.format(t)}
          </text>
        ))}

        {/* Area + line, per gap-free segment */}
        {segments.map((seg, i) => {
          // A segment isolated by gaps on both sides is a single sample — draw it
          // as a dot so an all-isolated window still shows its data (not a blank).
          if (seg.length === 1) {
            return <circle key={i} cx={xOf(seg[0].t)} cy={yOf(seg[0].v)} r="2.5" fill={stroke} />;
          }
          const line = seg.map((p) => `${xOf(p.t).toFixed(1)},${yOf(p.v).toFixed(1)}`).join(" ");
          const area = `${xOf(seg[0].t).toFixed(1)},${M.top + ih} ${line} ${xOf(
            seg[seg.length - 1].t,
          ).toFixed(1)},${M.top + ih}`;
          return (
            <g key={i}>
              <polygon points={area} fill={`url(#${gid})`} />
              <polyline
                points={line}
                fill="none"
                stroke={stroke}
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </g>
          );
        })}

        {/* Hover cursor + point */}
        {nearest && (
          <g>
            <line
              className="ts-cursor"
              x1={xOf(nearest.t)}
              x2={xOf(nearest.t)}
              y1={M.top}
              y2={M.top + ih}
            />
            <circle cx={xOf(nearest.t)} cy={yOf(nearest.v)} r="3.5" fill={stroke} />
          </g>
        )}
      </svg>

      {nearest && (
        <div
          className="ts-tip"
          style={{
            left: Math.min(Math.max(xOf(nearest.t), M.left), width - M.right),
          }}
        >
          <span className="ts-tip__v">
            {format(nearest.v)}
            {unit && <span className="ts-tip__u">{unit}</span>}
          </span>
          <span className="ts-tip__t">{stampFmt.format(nearest.t)}</span>
        </div>
      )}
    </div>
  );
}
