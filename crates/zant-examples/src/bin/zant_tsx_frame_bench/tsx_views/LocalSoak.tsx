// Soak-only local_bench.tsx: clear unmounts every stateful child.
// Keep the normal frame benchmark page and its output unchanged.
import { useState, useTheme, View, Row, Text, Button } from "zant";
import type { u32 } from "zant";

interface CounterProps {
  id: u32;
  odd: boolean;
}

function Counter({ id, odd }: CounterProps) {
  const theme = useTheme();
  const [count, setCount] = useState<u32>(0);
  return (
    <Row
      gap={8}
      alignItems="center"
      height={36}
      background={odd ? theme.colorFillQuaternary : theme.colorBgContainer}
    >
      <Text>
        Row {id}: {count}
      </Text>
      <Button key={`inc-${id}`} onClick={() => setCount(count + 1)}>
        +1
      </Button>
    </Row>
  );
}

export default function LocalBench() {
  const [ids, setIds] = useState<u32[]>([]);
  const [next, setNext] = useState<u32>(1);

  function add() {
    setIds([...ids, next]);
    setNext(next + 1);
  }

  return (
    <View padding={12} gap={4}>
      <Button key="add" onClick={add}>
        Add row
      </Button>
      <Button key="clear" onClick={() => setIds([])}>Clear rows</Button>
      {ids.map((id) => (
        <Counter key={id} id={id} odd={id % 2 === 1} />
      ))}
    </View>
  );
}
