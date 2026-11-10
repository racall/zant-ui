import { useState, View, Row, Text, Button, Input, List } from "zant";
import type { u32 } from "zant";

interface CounterProps {
  label: string;
}

// local state: each <Counter> instance has its own count and draft
function Counter({ label }: CounterProps) {
  const [count, setCount] = useState(0);
  const [draft, setDraft] = useState("");
  const [log, setLog] = useState<string[]>([]);

  function add() {
    setLog([...log, draft]);
    setDraft("");
  }

  return (
    <Row gap={8} alignItems="center">
      <Text>
        {label}: {count} ({log.length})
      </Text>
      <Button key={`inc-${label}`} onClick={() => setCount((c) => c + 1)}>
        +
      </Button>
      <Input key={`draft-${label}`} value={draft} onChange={setDraft} onSubmit={add} />
    </Row>
  );
}

// no state of its own, but renders a stateful child: takes an instance id
function Section({ label }: CounterProps) {
  return (
    <View gap={4}>
      <Counter label={label} />
      <Counter label={`${label} 2`} />
    </View>
  );
}

// a <List> in a child keeps its scroll state locally
function Feed() {
  return <List data={[1, 2, 3]} renderItem={(n) => <Text>{n}</Text>} />;
}

// stateless: plain builder, no id
function Title({ label }: CounterProps) {
  return <Text>{label}</Text>;
}

export default function App() {
  const [ids, setIds] = useState<u32[]>([1, 2]);
  return (
    <View padding={16} gap={8}>
      <Title label="Counters" />
      {ids.map((id) => (
        <Counter key={id} label={`#${id}`} />
      ))}
      <Section label="Section" />
      <Feed />
      <Button key="drop" onClick={() => setIds(ids.slice(1))}>
        Drop first
      </Button>
    </View>
  );
}
