interface Box { title: string; values: number[]; nested: { enabled: boolean } }
type Mode = "all" | "type";
const PLANS: string[] = ["Free", "Pro", "Team"];
const BANNER = `Plans ${PLANS.length}`;
const ALL_PLANS: string[] = [...PLANS, "Enterprise"];
const NUMBERS = [1, 2, 3, 4];

export default function Arrays() {
  const [boxes, setBoxes] = useState<Box[]>([
    { title: "First", values: [1, 2], nested: { enabled: true } }
  ]);
  function lengthOf(box: { values: number[] }): number { return box.values.length; }
  const size = lengthOf({ values: NUMBERS });
  const mapped = NUMBERS.map((x, i) => { const doubled = x * 2; return doubled + i; });
  const filtered = NUMBERS.filter(x => x > 2);
  const found = NUMBERS.find(x => x > 2);
  const position = NUMBERS.findIndex(x => x === 3);
  const some = NUMBERS.some(x => x === 2);
  const every = NUMBERS.every(x => x > 0);
  const index = PLANS.indexOf("Pro");
  const includes = PLANS.includes(`Pro`);
  const section = PLANS.slice(1, 3);
  const tail = PLANS.slice(-2);
  const joined = section.join(" / ");
  const expanded = NUMBERS.concat([5, 6], 7);
  const sum = NUMBERS.reduce((acc, x) => acc + x, 0);
  const matrix: number[][] = [[1, 2], [...filtered, 8]];
  const first = matrix[0][1];
  const chosen: number | null = found;
  const fallback = chosen ?? 0;
  const certain = found!;
  const maybe: Box | undefined = boxes.find(b => b.nested.enabled);
  const title = maybe?.title ?? "None";
  const emptyMap = NUMBERS.map(() => 1);
  const keepAll = NUMBERS.filter(() => true);
  const firstFound = NUMBERS.find(() => true)!;
  const nulls = NUMBERS.map(x => { if (x > 2) return x; return null; });
  const optionalValues: number[] | null = some ? NUMBERS : null;
  const indexed = optionalValues?.[0] ?? 0;
  const mode: Mode = "type";
  const suffix = "mode=" + mode;
  const self = sum;
  const bytes: u8[] = [1, 2];
  const peer = (some ? 1 : 2) + bytes[0];
  return <View gap={8}>
    <Select options={PLANS} />
    <Text key="banner">{BANNER}</Text>
    <Text key="strings">{joined} {tail.join(",")} {title} {suffix}</Text>
    <Text key="results">{position} {some} {every} {index} {includes}</Text>
    <Text key="numbers">{sum} {first} {fallback} {certain}</Text>
    <Text key="lengths">{mapped.length} {expanded.length} {chosen === null}</Text>
    <Text key="peer">{peer} {size}</Text>
    <Text key="bounds">{ALL_PLANS.length} {indexed}</Text>
    <Text key="callbacks">{emptyMap.length} {keepAll.length}</Text>
    <Text key="nullable">{firstFound} {nulls[0] ?? 99}</Text>
    {some && <Text key="optional">Some</Text>}
    {some && mapped.map(x => <Text key={x}>{x}</Text>)}
    {some ? NUMBERS.map(x => x > 2 ? <Text key={`optional-${x}`}>{x}</Text> : null) : null}
    <Button key="capture" onClick={() => setBoxes(current => current.slice(0, self))}>Keep</Button>
    <Button key="update"
      onClick={() => setBoxes(current => current.map(b => ({ ...b, title: suffix })))}>Update</Button>
  </View>;
}
