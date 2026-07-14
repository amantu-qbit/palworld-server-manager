import { TECH_ATLAS_CELL, TECH_ATLAS_SIZE } from "../lib/techDex";

/**
 * Renders one technology icon from the sprite atlas
 * (`public/mapicons/tech-atlas.webp`) as a scaled background-position window.
 * `cell` is the (col,row) from `techCell`; null renders an empty placeholder.
 */
export function TechIcon({
  cell,
  size = 44,
}: {
  cell: { col: number; row: number } | null;
  size?: number;
}) {
  if (!cell) {
    return <div className="techicon techicon--empty" style={{ width: size, height: size }} />;
  }
  const scale = size / TECH_ATLAS_CELL;
  return (
    <div
      className="techicon"
      style={{
        width: size,
        height: size,
        backgroundImage: "url(/mapicons/tech-atlas.webp)",
        backgroundSize: `${TECH_ATLAS_SIZE * scale}px ${TECH_ATLAS_SIZE * scale}px`,
        backgroundPosition: `${-cell.col * TECH_ATLAS_CELL * scale}px ${-cell.row * TECH_ATLAS_CELL * scale}px`,
      }}
    />
  );
}
