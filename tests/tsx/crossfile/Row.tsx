// A row: its props use a type imported from Zig (data.zig).
import { Button } from "zant";
import type { i64 } from "zant";
import type { Data } from "./data.zig";
/// Exported constants import like functions (`_Row.rowPrefix`).
export const rowPrefix = "#";
export const maxLabel: number = 12;

export interface RowProps {
  entry: Data;
  onPick: (row: i64) => void;
}
export function Row({ entry, onPick }: RowProps) {
  return (
    <Button key={`pick-${entry.row}`} onClick={() => onPick(entry.row)}>
      {entry.label} {entry.kind === "image" ? "img" : "txt"}
    </Button>
  );
}
