import { useState, createStore, View, Text, Button } from "zant";

interface Prefs {
  unit: string;
}

const prefs = createStore<Prefs>({ unit: "KB" });

// plain functions: lowercase, typed parameters and a return type
function plural(n: number, word: string): string {
  return n === 1 ? `${n} ${word}` : `${n} ${word}s`;
}

function clamp(n: number, lo: number, hi: number): number {
  if (n < lo) return lo;
  if (n > hi) return hi;
  return n;
}

// recursion, and calling another function declared later
function digits(n: number): number {
  if (n < 10) return 1;
  return 1 + digits(n / 10);
}

const half = (n: number): number => n / 2;

// reads and changes a store; optional parameter
function size(n: number, unit?: string): string {
  return `${half(n * 2)} ${unit ?? prefs.get().unit}`;
}

function reset(): void {
  prefs.set({ unit: "KB" });
}

function Badge({ count }: { count: number }) {
  return <Text key="badge">{plural(count, "item")}</Text>;
}

export default function App() {
  const [count, setCount] = useState(0);
  const label = plural(count, "click");
  return (
    <View padding={16} gap={8}>
      <Text key="label">
        {label}, {digits(count * 100)} digits, {size(count)}
      </Text>
      <Badge count={clamp(count, 0, 3)} />
      <Button key="inc" onClick={() => setCount(clamp(count + 1, 0, 5))}>
        +1
      </Button>
      <Button key="unit" onClick={() => prefs.update({ unit: "MB" })}>
        MB
      </Button>
      <Button key="reset" onClick={reset}>
        Reset
      </Button>
    </View>
  );
}
