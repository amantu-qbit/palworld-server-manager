import { ATLAS_CELL, ATLAS_H, ATLAS_W } from "../lib/palDex";

/**
 * Renders one Pal from the sprite atlas (`public/mapicons/pals-atlas.webp`) as
 * a scaled background-position window. `cell` is the (col,row) from `palInfo`;
 * null renders an empty placeholder.
 */
export function PalIcon({
  cell,
  size = 48,
}: {
  cell: { col: number; row: number } | null;
  size?: number;
}) {
  if (!cell) {
    return <div className="palicon palicon--empty" style={{ width: size, height: size }} />;
  }
  const scale = size / ATLAS_CELL;
  return (
    <div
      className="palicon"
      style={{
        width: size,
        height: size,
        backgroundImage: "url(/mapicons/pals-atlas.webp)",
        backgroundSize: `${ATLAS_W * scale}px ${ATLAS_H * scale}px`,
        backgroundPosition: `${-cell.col * ATLAS_CELL * scale}px ${-cell.row * ATLAS_CELL * scale}px`,
      }}
    />
  );
}
