import type { ReactNode } from "react";

export type TableRow = Record<string, unknown>;

export interface TableColumn {
  key: string;
  header: ReactNode;
  /** Custom cell; defaults to `row[key]`. */
  render?: (row: TableRow) => ReactNode;
  align?: "start" | "center" | "end";
}

export interface TableProps {
  columns: readonly TableColumn[];
  rows: readonly TableRow[];
  /** Stable row key; falls back to the index. */
  getRowKey?: (row: TableRow, index: number) => string;
  caption?: string;
  /** Zebra striping for scanability. */
  striped?: boolean;
  className?: string;
}

/** Data table with a styled header, hover rows and an optional striped variant. */
export function Table({ columns, rows, getRowKey, caption, striped, className = "" }: TableProps) {
  return (
    <div className={`rumahl-table-wrap ${className}`}>
      <table className="rumahl-table" data-variant={striped ? "striped" : undefined}>
        {caption ? <caption>{caption}</caption> : null}
      <thead>
        <tr>
          {columns.map((column) => (
            <th key={column.key} scope="col" style={column.align ? { textAlign: column.align } : undefined}>
              {column.header}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row, index) => (
          <tr key={getRowKey ? getRowKey(row, index) : index}>
            {columns.map((column) => (
              <td key={column.key} style={column.align ? { textAlign: column.align } : undefined}>
                {column.render ? column.render(row) : (row[column.key] as ReactNode)}
              </td>
            ))}
          </tr>
        ))}
        </tbody>
      </table>
    </div>
  );
}
