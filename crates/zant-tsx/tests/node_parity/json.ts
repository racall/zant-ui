// JSON.stringify / JSON.parse: printed output must match Node.

interface Item {
  id: number;
  title: string;
  done: boolean;
  note?: string;
  owner: string | null;
}

type Shape = { kind: "circle"; r: number } | { kind: "rect"; w: number; h: number };

interface Doc {
  name: string;
  items: Item[];
  tags: Record<string, number>;
  shapes: Shape[];
  mode: "light" | "dark";
  value: string | number;
  point: [number, number];
}

function sample(): Doc {
  return {
    name: "清单 \"one\"\n\ttab",
    items: [
      { id: 1, title: "写文档", done: false, owner: null },
      { id: 2.5, title: "emoji 🎉", done: true, note: "n", owner: "ada" },
    ],
    tags: { zeta: 1, alpha: 2, "10": 3, "2": 4 },
    shapes: [
      { kind: "circle", r: 1.5 },
      { kind: "rect", w: 2, h: 3 },
    ],
    mode: "dark",
    value: 42,
    point: [0.1 + 0.2, 1e21],
  };
}

function numbers(): string {
  const values: number[] = [0, -0, 1.5, 1e21, 1e-7, 123456789012345680000, 0.1 + 0.2, NaN, Infinity, -Infinity];
  return JSON.stringify(values);
}

function parseErrors(): string {
  const inputs: string[] = ["", "{", "{\"a\":}", "[1,]", "tru", "01", "{\"a\" 1}", "\"\\x\"", "[1] 2", "{'a':1}"];
  let out = "";
  for (const text of inputs) {
    try {
      const value = JSON.parse(text) as number[];
      out += "ok " + value.length + "\n";
    } catch (e) {
      out += (e instanceof SyntaxError ? "SyntaxError: " + e.message : "other") + "\n";
    }
  }
  return out;
}

export function run(): void {
  const doc = sample();
  console.log(JSON.stringify(doc));
  console.log(JSON.stringify(doc, null, 2));
  console.log(JSON.stringify(doc.items, null, "\t"));
  console.log(JSON.stringify("a\u0001b\u007f"));
  console.log(numbers());
  const empty: number[] = [];
  const nothing: Record<string, number> = {};
  console.log(JSON.stringify(empty), JSON.stringify(nothing), JSON.stringify(true), JSON.stringify(null));
  const back = JSON.parse(JSON.stringify(doc)) as Doc;
  console.log(back.items[1].title, back.items[1].note ?? "none", back.items[0].note ?? "none");
  console.log(back.items[0].owner ?? "nobody", back.tags["alpha"] ?? 0, back.point[0]);
  for (const shape of back.shapes) {
    if (shape.kind === "circle") {
      console.log("circle", shape.r);
    } else {
      console.log("rect", shape.w * shape.h);
    }
  }
  console.log(Object.keys(back.tags).join(","));
  const list: number[] = JSON.parse(" [1, 2.5e2 , -3] ");
  console.log(list.length, list[1]);
  console.log(parseErrors());
}
