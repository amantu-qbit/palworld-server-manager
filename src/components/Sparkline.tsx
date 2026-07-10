interface Props {
  points: number[];
  width?: number;
  height?: number;
  stroke?: string;
  fill?: boolean;
}

/** Minimal SVG sparkline that auto-scales to the data range. */
export function Sparkline({ points, width = 120, height = 28, stroke = "var(--accent)", fill = true }: Props) {
  if (points.length < 2) {
    return <svg width="100%" height={height} viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" />;
  }
  const min = Math.min(...points);
  const max = Math.max(...points);
  const span = max - min || 1;
  const stepX = width / (points.length - 1);
  const pad = 3;
  const usable = height - pad * 2;

  const coords = points.map((p, i) => {
    const x = i * stepX;
    const y = pad + (1 - (p - min) / span) * usable;
    return [x, y] as const;
  });

  const line = coords.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(" ");
  const area = `${coords[0][0]},${height} ${line} ${coords[coords.length - 1][0]},${height}`;
  const gid = `spark-${stroke.replace(/[^a-z0-9]/gi, "")}`;

  return (
    <svg width="100%" height={height} viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" aria-hidden>
      <defs>
        <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={stroke} stopOpacity="0.22" />
          <stop offset="1" stopColor={stroke} stopOpacity="0" />
        </linearGradient>
      </defs>
      {fill && <polygon points={area} fill={`url(#${gid})`} />}
      <polyline points={line} fill="none" stroke={stroke} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
