interface Props {
  width?: number | string;
  height?: number | string;
  radius?: number | string;
  style?: React.CSSProperties;
}

/** Shimmering placeholder block for loading states. */
export function Skeleton({ width = "100%", height = 14, radius = "var(--r-sm)", style }: Props) {
  return <div className="skeleton" style={{ width, height, borderRadius: radius, ...style }} />;
}
