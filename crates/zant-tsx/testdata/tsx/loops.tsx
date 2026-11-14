type Mode = "a" | "b";
const NUMBERS = [1, 2, 3, 4, 5];

function Sum({ values }: { values: number[] }) {
  function sum(): number {
    let total = 0;
    for (const x of values) total += x;
    return total;
  }
  return <Text key="props">{sum()}</Text>;
}

export default function App() {
  const [result, setResult] = useState(0);
  const [mounted, setMounted] = useState(0);
  const [items, setItems] = useState<number[]>([1, 2, 3, 4, 5]);
  const visits = useRef(0);
  useEffect(() => {
    let total = 0;
    for (const x of items) total += x;
    setMounted(total);
  }, [items]);
  function summed(n: number): number {
    let total = 0;
    for (let i = 0; i < n; i++) {
      if (i === 2) continue;
      if (i === 5) return total;
      total += i;
    }
    return total;
  }
  function numbers(): number[] { visits.current++; return [1, 2, 3]; }
  function next(n: number): number { visits.current++; return n; }
  function choose(mode: Mode): number {
    switch (mode) {
      case "a": return 1;
      case "b": return 7;
      default: return 0;
    }
  }
  function classic() {
    let total = 0;
    for (let i = 0; i < 8; i++) {
      if (i === 2) continue;
      if (i === 5) break;
      total += i;
    }
    setResult(total);
  }
  function each() {
    let total = 0;
    for (let x of items) { x++; total += x; }
    for (const x of [1, 2]) total += x;
    const xs = numbers();
    for (const x of xs) total += x;
    for (const x of numbers()) total += x - x;
    for (const unused of [1]) {}
    setResult(total);
  }
  function conditions() {
    let total = 0;
    while (total < 3) total++;
    do { total++; if (total === 4) continue; } while (total < 5);
    do total++; while (false)
    let i = 0;
    for (;;) { const unused = 1; break; }
    for (i = 0; i < 3; i++) total += i;
    for (; i > 0;) i--;
    setResult(total + i);
  }
  function select() {
    visits.current = 0;
    let total = 0;
    switch (next(result === 33 ? 9 : 2)) {
      case next(1): total = 1; break;
      default: total = 90; break;
      case next(2): case next(3): total = 20; break;
    }
    switch (result === 33 ? "unknown" : "go") {
      case "no": break;
      case "go": total += 2; break;
      default: case "other": const unused = 0; total += 2;
    }
    switch (true) { case false: break; case true: total += 3; }
    for (let i = 0; i < 4; i++) {
      switch (i) {
        case 0: continue;
        case 1: total++; break;
        default: break;
      }
      if (i === 2) break;
    }
    setResult(total + choose("b"));
  }
  const visible = NUMBERS.filter(x => {
    for (const candidate of [2, 4]) {
      if (candidate === x) return true;
    }
    return false;
  });
  return <View gap={8} padding={12}>
    <Text key="result">{result}</Text>
    <Text key="mounted">{mounted}</Text>
    <Text key="filtered">{visible.length}</Text>
    <Text key="visits">{visits.current}</Text>
    <Sum values={items}/>
    <Button key="classic" onClick={classic}>For</Button>
    <Button key="each" onClick={each}>For of</Button>
    <Button key="conditions" onClick={conditions}>While and do</Button>
    <Button key="select" onClick={select}>Switch</Button>
    <Button key="sum" onClick={() => setResult(summed(8))}>Function</Button>
    <Button key="items" onClick={() => setItems([2, 4])}>Effect</Button>
  </View>;
}
