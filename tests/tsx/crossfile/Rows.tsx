// Rows: uses Row (props typed in another file) with the same imported type.
import { View } from "zant";
import type { i64 } from "zant";
import type { Data } from "./data.zig";
import { Row } from "./Row";
export interface RowsProps {
  items: Data[];
  onPick: (row: i64) => void;
}
export function Rows({ items, onPick }: RowsProps) {
  return <View>{items.map((d) => <Row key={d.row} entry={d} onPick={onPick} />)}</View>;
}
