// `examples/tsx/list_bench.tsx` with the 10 000 rows built by a same-file
// function instead of injected through `tsx.State`, for the TSX frame
// benchmark (`zant-tsx-frame-bench`, WP-5.10c).
//
// The Zig benchmark filled the root component's `rows` state from the driver
// (`app.rows.set(tsx_rows)`); Rust codegen keeps a root component's fields
// private, so the page builds the identical 10 000 `Row title` rows itself.
// `useMemo` runs `makeRows` once, on the untimed warm-up frame of the harness,
// so the per-frame cost is the real list rendering. Everything else below is
// `list_bench.tsx` verbatim: same `<List>` JSX, keys, `renderItem`,
// `keyExtractor` and row styling.
//
// (A prop-taking `ListView` component was tried first and hit a `zant-tsx`
// codegen bug: a non-root component rendering `<List>` moves `self` into the
// item closure while `self._listN` is borrowed — reported in
// docs/rust-port/status.md WP-5.10c. The root form compiles.)
import { useMemo, useState, useTheme, List, View, Row, Text, Button } from "zant";
import type { u32 } from "zant";

interface RowData {
  id: u32;
  title: string;
}

// Phase 1 TSX has no array `push`; the 10 000 rows are built by doubling a
// small base array with `concat` (write-on-copy lists, O(n) overall) and then
// numbering them with `map`.
function makeRows(): RowData[] {
  let rows: RowData[] = [];
  for (let i = 0; i < 32; i++) {
    rows = rows.concat([{ id: 0, title: "Row title" }]);
  }
  for (let i = 0; i < 9; i++) {
    rows = rows.concat(rows);
  }
  return rows.slice(0, 10000).map((row, index) => ({ id: (index as u32) + 1, title: row.title }));
}

export default function ListBench() {
  const theme = useTheme();
  const rows = useMemo(() => makeRows(), []);
  const [picked, setPicked] = useState<u32>(0);

  return (
    <View flex={1} padding={12} gap={8}>
      <Text key="picked">Picked {picked}</Text>
      <List
        key="rows"
        data={rows}
        itemHeight={40}
        keyExtractor={(row) => row.id}
        renderItem={(row, index) => (
          <Row key={`row-${row.id}`} gap={8} alignItems="center" height={40} background={index % 2 === 0 ? theme.colorFillQuaternary : theme.colorBgContainer}>
            <Text>{row.title}</Text>
            <Button key={`pick-${row.id}`} onClick={() => setPicked(row.id)}>
              Pick
            </Button>
          </Row>
        )}
      />
    </View>
  );
}
