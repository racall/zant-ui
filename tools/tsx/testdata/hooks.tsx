import { useState, useRef, useMemo, useEffect, View, Row, Text, Button, Input } from "zant";
import type { u32 } from "zant";

interface Item {
  id: u32;
  title: string;
  done: boolean;
}

interface StatsProps {
  items: Item[];
}

// useMemo in a child: recomputed only when `items` changes (by value)
function Stats({ items }: StatsProps) {
  const done = useMemo(() => items.filter((i) => i.done).length, [items]);
  const renders = useRef(0);
  useEffect(() => {
    renders.current += 1;
  });
  return (
    <Text key="stats">
      {done} / {items.length} done
    </Text>
  );
}

export default function App() {
  const [items, setItems] = useState<Item[]>([]);
  const [query, setQuery] = useState("");
  const [loaded, setLoaded] = useState(false);
  const nextId = useRef<u32>(1);
  const lastQuery = useRef("");

  // a block body, two dependencies
  const visible = useMemo(() => {
    if (query.length === 0) return items;
    return items.filter((i) => i.title === query);
  }, [items, query]);

  // once, after the first build
  useEffect(() => {
    setItems([{ id: 0, title: "First", done: true }]);
    setLoaded(true);
  }, []);

  // whenever the query changes
  useEffect(() => {
    lastQuery.current = query;
  }, [query]);

  function add() {
    setItems([...items, { id: nextId.current, title: `Item ${nextId.current}`, done: false }]);
    nextId.current++;
  }

  return (
    <View padding={16} gap={8}>
      <Row gap={8}>
        <Input key="query" value={query} onChange={setQuery} placeholder="Filter" />
        <Button key="add" onClick={add}>
          Add
        </Button>
      </Row>
      <Text key="visible">
        {visible.length} shown{loaded ? "" : " (loading)"}
      </Text>
      <Stats items={items} />
    </View>
  );
}
