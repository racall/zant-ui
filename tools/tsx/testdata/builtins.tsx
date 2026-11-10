const pi = Math.PI;
const e = Math.E;
const maxSafe = Number.MAX_SAFE_INTEGER;
const epsilon = Number.EPSILON;
const parsed = Number(" 0x1f ");

function probe(a: number, b: number): number {
  return Math.max(a, b);
}

export default function App() {
  const [count, setCount] = useState(2.5);
  const wide: u64 = 9007199254740993;
  const narrow: u8 = 7;
  const signed: i32 = -9;
  const exact: u64 = Math.max(wide, narrow);
  const absolute: i32 = Math.abs(signed);
  const floored: u64 = Math.floor(wide);
  const ceiled: u64 = Math.ceil(wide);
  const rounded: u64 = Math.round(wide);
  const truncated: u64 = Math.trunc(wide);
  const mixed: i32 = Math.min(narrow, signed);
  const allMath = [Math.sign(-2), Math.sqrt(9), Math.cbrt(-8), Math.pow(2, 3),
    Math.exp(0), Math.log(1), Math.log2(8), Math.log10(100), Math.sin(0),
    Math.cos(0), Math.tan(0), Math.atan(0), Math.atan2(0, 1), Math.hypot(3, 4)];
  const random = Math.random();
  const validRandom = random >= 0 && random < 1;
  const now = Date.now();
  const clock = performance.now();
  const binary = parseInt("101tail", 2);
  const bad = Number("bad");
  const finite = Number.isFinite(Math.min());
  return <View gap={4}>
    <Text key="rounding">{Math.floor(count)}/{Math.ceil(count)}/{Math.round(-count)}/{Math.trunc(-count)}</Text>
    <Text key="fixed">{(0.995).toFixed(2)}/{(9.999).toFixed(2)}/{count.toFixed()}</Text>
    <Text key="integers">{exact}/{absolute}/{mixed}</Text>
    <Text key="identities">{floored}/{ceiled}/{rounded}/{truncated}</Text>
    <Text key="conversion">{Number(true)}/{Number(false)}/{Number(" ")}/{parsed}/{Number(count)}</Text>
    <Text key="parse">{parseInt(" -0x1f!")}/{binary}/{parseInt("z!", 36)}/{parseFloat(" 1.25e2x")}</Text>
    <Text key="special">{String(Number("bad"))}/{Math.min()}/{Math.max()}/{String(-0)}</Text>
    <Text key="format">{String(1e21)}/{(1e-7).toString()}/{`${1e20}/${1e-6}/${count}`}</Text>
    <Text key="predicates">{Number.isNaN(bad)}/{Number.isNaN("bad")}/{finite}/{Number.isInteger(2)}</Text>
    <Text key="constants">{pi > 3}/{e > 2}/{maxSafe}/{epsilon}</Text>
    <Text key="math">{allMath.map(n => String(n)).join(",")}/{probe(3, 8)}</Text>
    <Text key="clock">{now > 1700000000000}/{clock >= 0}/{validRandom}</Text>
    <Button key="change" onClick={() => setCount(1.25)}>Change</Button>
  </View>;
}
