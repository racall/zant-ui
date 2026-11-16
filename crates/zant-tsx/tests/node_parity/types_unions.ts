// docs/tsx.md:206,211: primitive/discriminated unions and exhaustive switches.
type Value = string | number | boolean;
type Circle = { kind: "circle"; size: number; r: number };
type Square = { kind: "square"; size: number; w: number };
type Shape = Circle | Square;
type Mode = "off" | "on" | "auto";

function primitiveIf(x: Value): string {
  if (typeof x === "string") return "s:" + x.toUpperCase();
  if (typeof x === "number") return "n:" + String(x + 1);
  if (typeof x === "boolean") return "b:" + String(!x);
  return "unreachable";
}
function primitiveSwitch(x: Value): string {
  switch (typeof x) {
    case "string": return x.trim();
    case "number": return String(x * 2);
    case "boolean": return x ? "yes" : "no";
  }
}
function primitiveExpr(x: string | number): string {
  const long = typeof x === "string" && x.length > 1;
  return String(long) + ":" + (typeof x === "string" ? x.toUpperCase() : String(x + 2));
}
function shapeIf(s: Shape): number {
  if (s.kind === "circle") return s.r * s.r;
  return s.w * s.w;
}
function shapeExpr(s: Shape): string {
  const large = s.kind === "circle" && s.r > 2;
  return String(large) + ":" + String(s.kind === "circle" ? s.r : s.w);
}
function shapeSwitch(s: Shape): number {
  switch (s.kind) {
    case "circle": return s.r + s.size;
    case "square": return s.w + s.size;
  }
}
function modeCode(mode: Mode): number {
  switch (mode) {
    case "off": return 0;
    case "on": return 1;
    case "auto": return 2;
  }
}
export function run(): void {
  const values: Value[] = [" ab ", "", 0, 4, false, true];
  for (const value of values) {
    console.log("primitive", typeof value, primitiveIf(value), primitiveSwitch(value));
  }
  console.log("primitive-expr", primitiveExpr("ab"), primitiveExpr(""), primitiveExpr(0), primitiveExpr(4));
  const shapes: Shape[] = [
    { kind: "circle", size: 10, r: 3 },
    { kind: "square", size: 20, w: 4 }
  ];
  for (const s of shapes) console.log("shape", s.kind, s.size, shapeIf(s), shapeExpr(s), shapeSwitch(s));
  console.log("modes", modeCode("off"), modeCode("on"), modeCode("auto"));
}
