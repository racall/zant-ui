// More supported TypeScript in combination: printed output must match Node.

interface Box<T> {
  value: T;
  label: string;
}

interface Named {
  name: string;
}
type Point = Named & { x: number; y: number };

const LABELS = { open: "Open", done: "Done", hold: "On hold" } as const;
type Status = keyof typeof LABELS;

const PALETTE = { red: 1, green: 2 } satisfies Record<string, number>;

function wrap<T>(value: T, label: string): Box<T> {
  return { value, label };
}

function apply(f: (x: number) => number, values: number[]): number[] {
  return values.map((x) => f(x));
}

function compose(f: (x: number) => number, g: (x: number) => number): (x: number) => number {
  return (x: number): number => g(f(x));
}

function divide(a: number, b: number): number {
  if (b === 0) throw new RangeError("division by zero");
  return a / b;
}

function attempt(a: number, b: number): string {
  const log: string[] = [];
  try {
    log.push("try");
    const r = divide(a, b);
    log.push("ok " + r);
    return log.join(",");
  } catch (e) {
    log.push(e instanceof RangeError ? "range " + e.message : "other");
    return log.join(",");
  } finally {
    log.push("finally");
    console.log("finally ran for", a, b, log.length);
  }
}

function loopWithFinally(n: number): number {
  let count = 0;
  for (let i = 0; i < n; i++) {
    try {
      if (i === 2) continue;
      if (i === 4) break;
      count += 10;
    } finally {
      count += 1;
    }
  }
  return count;
}

function words(text: string): string[] {
  return text
    .split(/\s+/)
    .map((w) => w.replace(/[.,!?]/g, "").toLowerCase())
    .filter((w) => w.length > 0);
}

function frequency(text: string): [string, number][] {
  const counts = new Map<string, number>();
  for (const w of words(text)) {
    counts.set(w, (counts.get(w) ?? 0) + 1);
  }
  const entries: [string, number][] = [...counts.entries()];
  entries.sort((a, b) => b[1] - a[1]);
  return entries.slice(0, 3);
}

function unique(values: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const v of values) {
    if (!seen.has(v)) {
      seen.add(v);
      out.push(v);
    }
  }
  return out;
}

function edit(list: number[]): string {
  const copy = [...list];
  copy.push(9);
  copy.unshift(0);
  const removed = copy.splice(2, 2, 7, 7, 7);
  copy.reverse();
  const last = copy.pop() ?? -1;
  return `${copy.join("-")} removed=${removed.join("+")} last=${last}`;
}

function dates(log: string): string[] {
  const out: string[] = [];
  for (const m of log.matchAll(/(\d{4})-(\d{2})-(\d{2})/g)) {
    out.push(`${m[3]}/${m[2]}/${m[1]}@${m.index}`);
  }
  return out;
}

function title(text: string | null): string {
  return text?.trim().toUpperCase() ?? "UNTITLED";
}

function numbers(): string {
  return [
    (1.005).toFixed(2),
    (2.5).toFixed(0),
    String(Number("  42  ")),
    String(Number("0x1F")),
    String(Number("abc")),
    String(parseInt("12px")),
    String(parseInt("ff", 16)),
    String(parseFloat("3.5e2x")),
    String(Math.round(-2.5)),
    String(Math.max()),
    String(Math.hypot(3, 4)),
    String(0.1 + 0.2),
    String(1e21),
    (255).toString(),
  ].join(" ");
}

export function run(): void {
  const b = wrap([1, 2, 3], "nums");
  console.log(b.label, b.value.length, wrap("x", "str").value);
  const p: Point = { name: "origin", x: 0, y: 0 };
  console.log(p.name, p.x + p.y);
  const s: Status = "hold";
  console.log(LABELS[s], Object.keys(LABELS).join("/"), PALETTE.green);
  console.log(apply((x: number): number => x * x, [1, 2, 3]).join(","));
  const inc = (x: number): number => x + 1;
  const double = (x: number): number => x * 2;
  console.log(compose(inc, double)(5), compose(double, inc)(5));
  console.log(attempt(6, 3));
  console.log(attempt(1, 0));
  console.log(loopWithFinally(6));
  const text = "The cat and the hat. The end, and the cat!";
  console.log(frequency(text).map((e) => e[0] + ":" + e[1]).join(" "));
  console.log(unique(words(text)).join(","));
  console.log(edit([1, 2, 3, 4]));
  console.log(dates("from 2024-01-05 to 2025-12-31.").join(" | "));
  console.log(title("  hello "), title(null));
  console.log(numbers());
  const point: [string, number] = ["x", 3];
  const [axis, value] = point;
  console.log(axis, value, point.length);
  console.log("a-b_c d".replace(/[-_ ]/g, (m) => (m === " " ? "+" : "_")));
  console.log("Hello".padStart(8, "*"), "abc".at(-1) ?? "", "x,y,,z".split(",").length, " t ".trimEnd() + "|");
}
