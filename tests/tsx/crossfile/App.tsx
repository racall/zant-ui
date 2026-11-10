// Types across files (tests/tsx_crossfile_test.zig): a props interface using a
// type imported from Zig, and components that pass it on to each other.
import { useState, View, Text } from "zant";
import type { i64 } from "zant";
import { sample } from "./data.zig";
import { Row } from "./Row";
import { Rows } from "./Rows";
export default function App() {
  const [picked, setPicked] = useState<i64>(0);
  return (
    <View>
      <Row key="first" entry={sample()[0]} onPick={(r) => setPicked(r)} />
      <Rows key="all" items={sample()} onPick={(r) => setPicked(r)} />
      <Text key="picked">{picked}</Text>
    </View>
  );
}
