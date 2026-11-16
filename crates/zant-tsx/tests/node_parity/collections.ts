// Array and collection details found by the built-in probes: reduce with
// no initial value (and its TypeError), join's default separator, the
// fromIndex of indexOf / includes, Set.entries, and -0 keys
// (SameValueZero). Printed output must match Node.

function total(values: number[]): number {
  return values.reduce((a, b) => a + b);
}

function safeTotal(values: number[]): string {
  try {
    return String(total(values));
  } catch (e) {
    return e instanceof TypeError ? "TypeError: " + e.message : "other";
  }
}

export function run(): void {
  console.log([7].reduce((a, b) => a - b), total([1, 2, 3]), safeTotal([]), safeTotal([4, 5]));
  console.log([5, 1, 9].reduce((max, v, i) => (v > max ? v + i : max)));
  console.log(["a", "b"].join(), [1, 2.5, 3].join(), [true, false].join());
  const xs = [4, 5, 4, 6];
  console.log(xs.indexOf(4, 1), xs.indexOf(4, 3), xs.indexOf(6, -1), xs.indexOf(4, -99), xs.indexOf(5, 99));
  console.log(xs.includes(4, 1), xs.includes(5, 2), xs.includes(6, -1), [NaN].includes(NaN, 0));
  const s = new Set<string>(["a", "b"]);
  console.log(JSON.stringify([...s.entries()]));
  const m = new Map<number, string>([[-0, "x"]]);
  const z = new Set<number>([-0]);
  for (const k of m.keys()) console.log(String(1 / k));
  for (const k of z.values()) console.log(String(1 / k), z.has(0), m.get(0) ?? "?");
}
