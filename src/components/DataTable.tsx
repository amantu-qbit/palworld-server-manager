import type { ReactNode } from "react";

export interface Column<T> {
  key: string;
  header: ReactNode;
  render: (row: T) => ReactNode;
  align?: "left" | "right" | "center";
  width?: string;
  sortable?: boolean;
}

interface Props<T> {
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  onRowClick?: (row: T) => void;
  sortKey?: string;
  sortDir?: "asc" | "desc";
  onSort?: (key: string) => void;
}

/** Generic OLED data table with optional sortable headers and row click. */
export function DataTable<T>({
  columns,
  rows,
  rowKey,
  onRowClick,
  sortKey,
  sortDir,
  onSort,
}: Props<T>) {
  return (
    <div className="dtable-wrap">
      <table className="dtable">
        <thead>
          <tr>
            {columns.map((c) => (
              <th
                key={c.key}
                style={{ textAlign: c.align ?? "left", width: c.width }}
                className={c.sortable ? "dtable__sortable" : undefined}
                onClick={c.sortable && onSort ? () => onSort(c.key) : undefined}
                aria-sort={
                  sortKey === c.key ? (sortDir === "asc" ? "ascending" : "descending") : undefined
                }
              >
                <span className="dtable__th">
                  {c.header}
                  {c.sortable && sortKey === c.key && (
                    <span className="dtable__arrow">{sortDir === "asc" ? "▲" : "▼"}</span>
                  )}
                </span>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr
              key={rowKey(row)}
              onClick={onRowClick ? () => onRowClick(row) : undefined}
              className={onRowClick ? "dtable__row--click" : undefined}
            >
              {columns.map((c) => (
                <td key={c.key} style={{ textAlign: c.align ?? "left" }}>
                  {c.render(row)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
