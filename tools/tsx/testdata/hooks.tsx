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

interface TickerProps {
  label: string;
}

// a cleanup in a child: runs when the ticker unmounts; it reads a copy of the
// prop taken by the effect, and the child's own ref
function Ticker({ label }: TickerProps) {
  const runs = useRef(0);
  useEffect(() => {
    const name = label;
    runs.current += 1;
    return () => {
      runs.current -= name.length;
    };
  }, []);
  return <Text key={`ticker-${label}`}>{label}</Text>;
}

export default function App() {
  const [items, setItems] = useState<Item[]>([]);
  const [query, setQuery] = useState("");
  const [loaded, setLoaded] = useState(false);
  const nextId = useRef<u32>(1);
  const lastQuery = useRef("");
  const cleanups = useRef(0);
  const [ticking, setTicking] = useState(true);

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

  // whenever the query changes; the cleanup of the previous run sees its query
  useEffect(() => {
    const previous = query;
    lastQuery.current = query;
    return () => {
      cleanups.current += previous.length;
    };
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
      {ticking && <Ticker label="tick" />}
      <Button key="ticking" onClick={() => setTicking(!ticking)}>
        Toggle ticker
      </Button>
    </View>
  );
}
