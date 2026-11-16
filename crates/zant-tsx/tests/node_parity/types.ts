// Type details found by the type probes: `satisfies` keeps the value's own
// type, string unions are structural (keyof aliases mix), and an object on
// the left of && is truthy. Printed output must match Node.

interface Base {
  id: number;
}

interface Labels {
  a: string;
  b: string;
}

const labels: Labels = { a: "A", b: "B" };
type Key = keyof Labels;
type ConstantKey = keyof typeof labels;
type Short = "a";

function read(k: Key): string {
  return labels[k];
}

export function run(): void {
  const source = { id: 1, extra: 2 };
  const checked = source satisfies Base;
  console.log(JSON.stringify(checked), checked.extra);
  const k: ConstantKey = "b";
  const s: Short = "a";
  console.log(read(k), read(s));
  const p = { name: "Ada", tags: ["x"] };
  const list = [1, 2];
  console.log(p && p.name, list && list.length, p && p.tags.length > 0);
}
