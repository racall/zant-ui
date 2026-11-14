// Rows: uses Row (props typed in another file) with the same imported type,
// in a virtual list, forwarding its own callback prop (`props.onPick`).
import { List } from "zant";
import type { i64 } from "zant";
import type { Data } from "./data.rs";
import { Row } from "./Row";

export interface RowsProps {
  items: Data[];
  onPick: (row: i64) => void;
}

export function Rows(props: RowsProps) {
  return (
    <List
      data={props.items}
      keyExtractor={(d) => d.row}
      itemHeight={40}
      height={120}
      renderItem={(d) => <Row entry={d} onPick={props.onPick} />}
    />
  );
}
