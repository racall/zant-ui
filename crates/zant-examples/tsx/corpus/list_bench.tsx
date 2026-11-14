// A 10 000-row virtual list in TSX for the TSX frame benchmark (scenario
// `list`), which compares it with the same list written by hand in Rust.
import { useState, useTheme, List, View, Row, Text, Button } from "zant";
import type { u32 } from "zant";

interface Row {
  id: u32;
  title: string;
}

export default function ListBench() {
  const theme = useTheme();
  const [rows, setRows] = useState<Row[]>([]);
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
