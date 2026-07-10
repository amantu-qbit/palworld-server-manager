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

/** Animated radial ring gauge. Cyan stroke with a soft glow on OLED black. */
export function Gauge({ value, max, label, display, unit, size = 190, color = "var(--accent)" }: Props) {
  const stroke = 14;
  const r = (size - stroke) / 2 - 6;
  const c = 2 * Math.PI * r;
  const offset = dashoffset(value, max, c);
  const cx = size / 2;

  return (
    <div className="gauge" style={{ width: size, height: size }}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} role="img" aria-label={`${label}: ${display ?? value}`}>
        <defs>
          <linearGradient id="gauge-grad" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="var(--accent)" />
            <stop offset="1" stopColor="var(--accent-2)" />
          </linearGradient>
          <filter id="gauge-glow" x="-40%" y="-40%" width="180%" height="180%">
            <feGaussianBlur stdDeviation="4" result="b" />
            <feMerge>
              <feMergeNode in="b" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>
        <circle cx={cx} cy={cx} r={r} fill="none" stroke="var(--surface-3)" strokeWidth={stroke} />
        <motion.circle
          cx={cx}
          cy={cx}
          r={r}
          fill="none"
          stroke={color === "var(--accent)" ? "url(#gauge-grad)" : color}
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={c}
          transform={`rotate(-90 ${cx} ${cx})`}
          filter="url(#gauge-glow)"
          initial={{ strokeDashoffset: c }}
          animate={{ strokeDashoffset: offset }}
          transition={{ duration: 0.9, ease: [0.22, 1, 0.36, 1] }}
        />
      </svg>
      <div className="gauge__label">
        <div className="gauge__val mono">{display ?? Math.round(value)}</div>
        {unit && <div className="gauge__unit">{unit}</div>}
      </div>
    </div>
  );
}
