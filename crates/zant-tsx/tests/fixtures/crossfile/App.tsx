// Types across files: a props interface using a
// type imported from Rust, and components that pass it on to each other.
import { useState, View, Text } from "zant";
import type { i64 } from "zant";
import { sample } from "./data.rs";
import { Row, rowPrefix, maxLabel } from "./Row";
import { Rows } from "./Rows";
export default function App() {
  const [picked, setPicked] = useState<i64>(0);
  return (
    <View>
      <Row key="first" entry={sample()[0]} onPick={(r) => setPicked(r)} />
      <Rows key="all" items={sample()} onPick={setPicked} />
      <Text key="picked">{picked}</Text>
      {picked == 0 ? <Text key="none">none</Text> : sample().map((d) => <Text key={`label-${d.row}`}>{d.label}</Text>)}
      <Text key="consts">
        {rowPrefix}
        {maxLabel}
      </Text>
    </View>
  );
}
