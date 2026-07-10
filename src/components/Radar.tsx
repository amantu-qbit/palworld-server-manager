import { useMemo } from "react";
import type { Actor } from "../types/api";
import { computeBounds, projectToRadar } from "../lib/mapProject";

interface Props {
  actors: Actor[];
  visible: Set<string>;
  size?: number;
  onHover?: (a: Actor | null) => void;
}

interface DotStyle {
  color: string;
  r: number;
  glow?: boolean;
}

const STYLES: Record<string, DotStyle> = {
  Player: { color: "var(--accent)", r: 4, glow: true },
  WildPal: { color: "var(--faint)", r: 2.5 },
  BaseCampPal: { color: "var(--good)", r: 3 },
  OtomoPal: { color: "var(--accent-2)", r: 3 },
  NPC: { color: "var(--mute)", r: 2.5 },
};

const FALLBACK: DotStyle = { color: "var(--dim)", r: 2.5 };

/** Top-down radar of every actor, projected into a fixed square with north up. */
export function Radar({ actors, visible, size = 560, onHover }: Props) {
  const half = size / 2;
  const rings = [0.94, 0.63, 0.32];

  // Bounds use the full list so filtering points doesn't shift the projection.
  const bounds = useMemo(() => computeBounds(actors), [actors]);

  const points = useMemo(
    () =>
      actors
        .filter((a) => visible.has(a.UnitType))
        .map((a, i) => {
          const p = projectToRadar(a.LocationX, a.LocationY, bounds, size);
          const style = STYLES[a.UnitType] ?? FALLBACK;
          return { key: `${a.InstanceID || a.UnitType}-${i}`, actor: a, x: p.x, y: p.y, style };
        }),
    [actors, visible, bounds, size],
  );

  return (
    <div className="radar">
      <svg
        className="radar__svg"
        viewBox={`0 0 ${size} ${size}`}
        width="100%"
        role="img"
        aria-label="Live actor radar"
      >
        {rings.map((f) => (
          <circle key={f} className="radar__ring" cx={half} cy={half} r={half * f} fill="none" />
        ))}
        <line className="radar__cross" x1={half} y1={0} x2={half} y2={size} />
        <line className="radar__cross" x1={0} y1={half} x2={size} y2={half} />

        {points.map((p) => (
          <g key={p.key}>
            {p.style.glow && <circle cx={p.x} cy={p.y} r={p.style.r + 5} fill={p.style.color} opacity={0.18} />}
            <circle
              className="radar__dot"
              cx={p.x}
              cy={p.y}
              r={p.style.r}
              fill={p.style.color}
              onMouseEnter={() => onHover?.(p.actor)}
              onMouseLeave={() => onHover?.(null)}
            />
          </g>
        ))}
      </svg>
    </div>
  );
}
