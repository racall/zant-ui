// docs/tsx.md:207-211: type operators, tuples, structural types and generics.
interface Labels { short: string; long: string }
const labels: Labels = { short: "S", long: "Long" };
type LabelKey = keyof Labels;
type ConstantKey = keyof typeof labels;
type LabelValue = Labels["short"];
interface Base { id: number }
interface Named { name: string }
interface Item extends Base, Named { note?: string }
type Ranked = Base & { rank: number };
type OptionalId = { id?: number } & Base;
interface Box<T> { value: T }
type Pair<A, B = A> = { first: A; second: B };
type Maybe<T> = T | null;
enum Level { Low = 2, Medium, High = 8, Next }
enum Color { Red = "red", Blue = "blue" }
const enum Flag { First = 4, Second }

function lookup(k: LabelKey): LabelValue { return labels[k]; }
function readItem(item: Item): string { return String(item.id) + ":" + item.name + ":" + (item.note ?? "none"); }
function first<T>(xs: T[]): T | null { return xs.length > 0 ? xs[0] : null; }
function nameOf<T extends Named>(x: T): string { return x.name; }
function makePair<A, B = A>(a: A, b: B): Pair<A, B> { return { first: a, second: b }; }
const identity = <T,>(x: T): T => x;
function sum(xs: readonly number[]): number { return xs.reduce((a, b) => a + b, 0); }
function empty<T = number>(): T[] { return []; }
function tupleResult(name: string): [string, number] { return [name, name.length]; }

export function run(): void {
  const key: ConstantKey = "long";
  const same: typeof labels = { short: "x", long: "xyz" };
  console.log("operators", lookup("short"), labels[key], same.short, same.long);
  const tuple: [name: string, score: number] = ["pair", 7];
  const [a, b] = tuple;
  console.log("tuple", tuple[0], tuple[1], tuple.length, a, b);
  const entries: [string, number][] = [["a", 1], ["b", 2]];
  for (const [k, v] of entries) console.log("entry", k, v);
  const point = { x: 3, y: 4 };
  const twin = { x: 6, y: 8 };
  console.log("inferred", point.x + twin.x, point.y + twin.y);
  const rich = { id: 1, name: "one", extra: 99 };
  const item: Item = rich;
  console.log("structural", readItem(rich), readItem(item));
  const changed: Item = { ...item, name: "two", note: "memo" };
  const overwritten = { ...point, x: 9 };
  console.log("spreads", readItem(changed), overwritten.x, overwritten.y);
  const { name: title, id: code }: Item = changed;
  console.log("rename", title, code);
  const ranked: Ranked = { id: 4, rank: 2 };
  const required: OptionalId = { id: 5 };
  console.log("intersection", ranked.id, ranked.rank, required.id);
  const boxed: Box<string> = { value: "boxed" };
  const nested: Box<Box<number>> = { value: { value: 42 } };
  const pair: Pair<number> = { first: 2, second: 3 };
  const mixed: Pair<string, number> = makePair("x", 9);
  const maybe: Maybe<string> = null;
  console.log("generic-types", boxed.value, nested.value.value, pair.first, pair.second, mixed.first, mixed.second, maybe ?? "empty");
  console.log("generic-functions", first<number>([8, 9]) ?? -1, first(["a", "b"]) ?? "none", first<number>([]) ?? -1);
  console.log("generic-constraints", nameOf(rich), identity(3), identity("id"));
  const numbers: readonly number[] = [1, 2, 3];
  const more: ReadonlyArray<number> = [4, 5];
  const literal = { name: "fixed", id: 7 } as const;
  const checked = { id: 8, name: "checked" } satisfies Item;
  const asserted = { id: 9, name: "asserted" } as Item;
  console.log("readonly-assertions", sum(numbers), sum(more), literal.name, readItem(checked), readItem(asserted));
  console.log("enums", Level.Low, Level.Medium, Level.High, Level.Next, Color.Red, Color.Blue, Flag.First, Flag.Second);
  const [resultName, resultLength] = tupleResult("tuple");
  console.log("tuple-result", resultName, resultLength);
  const spreadOrder: { x: number; y: number } = { ...point, ...twin, y: 99 };
  console.log("spread-order", spreadOrder.x, spreadOrder.y);
  const extra = { id: 10, name: "extra", detail: "retained" };
  const verified = extra satisfies Item;
  console.log("satisfies-preserves", JSON.stringify(verified), readItem(verified));
  const genericTuple: Box<[string, number]> = { value: ["inside", 6] };
  console.log("generic-composed", genericTuple.value[0], genericTuple.value[1], empty().length, empty<string>().length);
  const { note: memo }: Item = item;
  console.log("destructure-optional", memo ?? "none");
  const literalArray = [1, 2, 3] as const;
  console.log("as-const-array", literalArray.length, literalArray[0]);
  const digit: 1 | 2 = 2;
  console.log("numeric-literals", digit + 1);
}
