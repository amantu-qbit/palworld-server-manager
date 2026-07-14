import { ITEM_ATLAS_CELL, ITEM_ATLAS_H, ITEM_ATLAS_W, itemCell } from "../lib/itemDex";

/** Renders an item's sprite from the bundled item atlas by its `static_id`. */
export function ItemIcon({ staticId, size = 30 }: { staticId: string; size?: number }) {
  const cell = itemCell(staticId);
  if (!cell) {
    return <div className="itemicon itemicon--empty" style={{ width: size, height: size }} />;
  }
  const scale = size / ITEM_ATLAS_CELL;
  return (
    <div
      className="itemicon"
      style={{
        width: size,
        height: size,
        backgroundImage: "url(/mapicons/items-atlas.webp)",
        backgroundSize: `${ITEM_ATLAS_W * scale}px ${ITEM_ATLAS_H * scale}px`,
        backgroundPosition: `${-cell.col * ITEM_ATLAS_CELL * scale}px ${-cell.row * ITEM_ATLAS_CELL * scale}px`,
      }}
    />
  );
}
