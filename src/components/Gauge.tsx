import { motion } from "framer-motion";
import { dashoffset } from "../lib/gauge";

interface Props {
  value: number;
  max: number;
  label: string;
  /** Displayed big number (defaults to rounded `value`). */
  display?: string | number;
  unit?: string;
  size?: number;
  color?: string;
}

/**
 * Instrument-style radial dial: a fine tick ring, a thin cyan value arc,
 * and a mono readout. Restrained glow — reads as precision hardware.
 */
export function Gauge({ value, max, label, display, unit, size = 188, color = "var(--accent)" }: Props) {
  const stroke = 8;
  const cx = size / 2;
  const r = cx - 26;
  const c = 2 * Math.PI * r;
  const offset = dashoffset(value, max, c);

  // fine tick ring just outside the arc
  const tickCount = 44;
  const tickR = r + 13;
  const ticks = Array.from({ length: tickCount }, (_, i) => {
    const a = (i / tickCount) * Math.PI * 2 - Math.PI / 2;
    const major = i % 4 === 0;
    const len = major ? 7 : 4;
    const x1 = cx + Math.cos(a) * tickR;
    const y1 = cx + Math.sin(a) * tickR;
    const x2 = cx + Math.cos(a) * (tickR - len);
    const y2 = cx + Math.sin(a) * (tickR - len);
    return { x1, y1, x2, y2, major };
  });

  return (
    <div className="gauge" style={{ width: size, height: size }}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} role="img" aria-label={`${label}: ${display ?? value}`}>
        {ticks.map((t, i) => (
          <line
            key={i}
            x1={t.x1}
            y1={t.y1}
            x2={t.x2}
            y2={t.y2}
            stroke={t.major ? "var(--line-3)" : "var(--line)"}
            strokeWidth={t.major ? 1.4 : 1}
          />
        ))}
        <circle cx={cx} cy={cx} r={r} fill="none" stroke="var(--surface-3)" strokeWidth={stroke} />
        <motion.circle
          cx={cx}
          cy={cx}
          r={r}
          fill="none"
          stroke={color}
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={c}
          transform={`rotate(-90 ${cx} ${cx})`}
          initial={{ strokeDashoffset: c }}
          animate={{ strokeDashoffset: offset }}
          transition={{ duration: 0.9, ease: [0.22, 1, 0.36, 1] }}
          style={{ filter: "drop-shadow(0 0 5px var(--accent-glow))" }}
        />
      </svg>
      <div className="gauge__label">
        <div className="gauge__val mono">{display ?? Math.round(value)}</div>
        {unit && <div className="gauge__unit">{unit}</div>}
      </div>
    </div>
  );
}
