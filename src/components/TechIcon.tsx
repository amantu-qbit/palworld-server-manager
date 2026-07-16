import type { CSSProperties } from "react";
import { TECH_ATLAS_COLS, TECH_ATLAS_ROWS } from "../lib/techDex";

/**
 * Renders one technology icon from the sprite atlas
 * (`public/mapicons/tech-atlas.webp`). Sizing is driven entirely by the CSS
 * custom property `--isz` (set by the tile), so the icon scales responsively
 * with the tile; this component only supplies the atlas cell (`--tcol`/`--trow`)
 * and the atlas column count. `cell` null renders an empty placeholder.
 */
export function TechIcon({ cell }: { cell: { col: number; row: number } | null }) {
  if (!cell) {
    return <div className="techicon techicon--empty" />;
  }
  return (
    <div
      className="techicon"
      style={
        {
          "--tcol": cell.col,
          "--trow": cell.row,
          "--tcols": TECH_ATLAS_COLS,
          "--trows": TECH_ATLAS_ROWS,
        } as CSSProperties
      }
    />
  );
}
