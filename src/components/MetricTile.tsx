import type { LucideIcon } from "lucide-react";
import { Sparkline } from "./Sparkline";

interface Props {
  icon: LucideIcon;
  label: string;
  value: string | number;
  unit?: string;
  suffix?: string;
  delta?: { text: string; tone?: "good" | "bad" | "dim" };
  points?: number[];
  stroke?: string;
}

/** Compact KPI tile with an optional delta badge and sparkline. */
export function MetricTile({ icon: Icon, label, value, unit, suffix, delta, points, stroke }: Props) {
  const tone = delta?.tone ?? "good";
  const deltaColor = tone === "bad" ? "var(--bad)" : tone === "dim" ? "var(--faint)" : "var(--good)";
  return (
    <div className="tile metric">
      <div className="metric__head">
        <Icon size={13} />
        <span>{label}</span>
      </div>
      {delta && (
        <div className="metric__delta mono" style={{ color: deltaColor }}>
          {delta.text}
        </div>
      )}
      <div className="metric__value mono">
        {value}
        {unit && <small>{unit}</small>}
        {suffix && <small>{suffix}</small>}
      </div>
      {points && points.length > 1 && (
        <div className="metric__spark">
          <Sparkline points={points} stroke={stroke ?? "var(--accent)"} />
        </div>
      )}
    </div>
  );
}
