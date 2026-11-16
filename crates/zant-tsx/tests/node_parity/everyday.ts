// Everyday TypeScript in combination (types, unions, generics, Map, JSON,
// regex, sorting, .length arithmetic): printed output must match Node.

interface User {
  id: number;
  name: string;
  email?: string;
  roles: string[];
  managers: User[];
}

type Shape =
  | { kind: "circle"; r: number }
  | { kind: "rect"; w: number; h: number }
  | { kind: "tri"; a: number; b: number; c: number };

enum Level {
  Low = 1,
  Mid = 5,
  High = 10,
}

function area(s: Shape): number {
  switch (s.kind) {
    case "circle":
      return Math.round(Math.PI * s.r * s.r * 100) / 100;
    case "rect":
      return s.w * s.h;
    case "tri": {
      const p = (s.a + s.b + s.c) / 2;
      return Math.round(Math.sqrt(p * (p - s.a) * (p - s.b) * (p - s.c)) * 100) / 100;
    }
  }
}

function first<T>(items: T[]): T | undefined {
  return items.length > 0 ? items[0] : undefined;
}

function pair<A, B>(a: A, b: B): [A, B] {
  return [a, b];
}

function chain(user: User): string {
  const names: string[] = [];
  let current: User | null = user;
  while (current !== null) {
    names.push(current.name);
    current = current.managers.length > 0 ? current.managers[0] : null;
  }
  return names.join(" > ");
}

function describe(user: User): string {
  const { name, roles } = user;
  const email = user.email ?? "no-email";
  const primary = roles.length > 0 ? roles[0] : "none";
  return `${name} <${email}> ${primary} +${Math.max(0, roles.length - 1)} boss=${user.managers.length > 0 ? user.managers[0].name : "-"}`;
}

function sum(...values: number[]): number {
  return values.reduce((acc, v) => acc + v, 0);
}

function groupBy(words: string[]): Map<number, string[]> {
  const groups = new Map<number, string[]>();
  for (const w of words) {
    const list = groups.get(w.length) ?? [];
    groups.set(w.length, [...list, w]);
  }
  return groups;
}

function stats(values: number[]): { min: number; max: number; mean: number } {
  const sorted = [...values].sort((a, b) => a - b);
  const mean = values.length === 0 ? 0 : sum(...values) / values.length;
  const empty = sorted.length === 0;
  return { min: empty ? 0 : sorted[0], max: empty ? 0 : sorted[sorted.length - 1], mean };
}

function parseQuery(query: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const part of query.split("&")) {
    const kv = part.split("=");
    const key = kv[0];
    if (key) out[key.replaceAll("%20", " ")] = (kv.length > 1 ? kv[1] : "").replaceAll("%20", " ");
  }
  return out;
}

function safeParse(text: string): number {
  try {
    const value: { n: number } = JSON.parse(text);
    return value.n;
  } catch (e) {
    return e instanceof SyntaxError ? -1 : -2;
  }
}

function slug(title: string): string {
  return title
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function levelName(level: Level): string {
  return level >= Level.High ? "high" : level >= Level.Mid ? "mid" : "low";
}

export function run(): void {
  const boss: User = { id: 1, name: "Ada", roles: ["admin"], managers: [] };
  const dev: User = { id: 2, name: "Bob", email: "bob@x.io", roles: ["dev", "ops", "qa"], managers: [boss] };
  const intern: User = { id: 3, name: "Cy", roles: [], managers: [dev] };
  console.log(chain(intern));
  console.log(describe(dev));
  console.log(describe(intern));
  const shapes: Shape[] = [
    { kind: "circle", r: 2 },
    { kind: "rect", w: 3, h: 4 },
    { kind: "tri", a: 3, b: 4, c: 5 },
  ];
  console.log(shapes.map((s) => area(s)).join(","));
  console.log(shapes.filter((s) => s.kind !== "circle").length);
  console.log(first([7, 8]) ?? -1, first<number>([]) ?? -1);
  const [label, count] = pair("items", 3);
  console.log(label, count);
  console.log(sum(), sum(1, 2, 3));
  const groups = groupBy(["a", "bb", "cc", "ddd", "e"]);
  for (const [len, words] of groups) {
    console.log(len, words.join("|"));
  }
  const s = stats([5, 1, 9, 3]);
  console.log(s.min, s.max, s.mean);
  const q = parseQuery("name=Ada%20L&lang=ts&empty=");
  console.log(Object.keys(q).join(","), q["name"] ?? "?", (q["empty"] ?? "x") === "");
  console.log(safeParse('{"n": 42}'), safeParse("{bad"));
  console.log(slug("  Hello, World! TSX -> Rust  "));
  console.log(levelName(Level.Low), levelName(Level.Mid), levelName(Level.High));
  const users = [dev, boss, intern];
  const byName = users.map((u) => u.name).sort();
  console.log(byName.join(" "), users.some((u) => u.roles.includes("qa")), users.every((u) => u.id > 0));
  const index = users.findIndex((u) => u.name === "Cy");
  console.log(index, users.find((u) => u.id === 9)?.name ?? "missing");
  const totals = users.reduce((acc, u) => acc + u.roles.length, 0);
  console.log(totals, JSON.stringify({ id: boss.id, roles: boss.roles, nested: { ok: true } }));
}
