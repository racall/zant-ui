// JavaScript truthiness: printed output must match Node.

interface User {
  name: string;
  nick?: string;
}

function label(name: string): string {
  return name || "Untitled";
}

function count(n: number): string {
  if (n) {
    return "some " + n;
  }
  return "none";
}

function describe(user: User | null): string {
  const nick = user && user.nick;
  const shown = nick || (user && user.name) || "guest";
  return shown + (user ? "" : " (signed out)");
}

function flags(text: string, n: number, items: string[], maybe: string | null): string {
  const parts: string[] = [];
  parts.push(!text ? "empty" : "text");
  parts.push(!n ? "zero" : "nonzero");
  parts.push(items ? "array is truthy" : "never");
  parts.push(maybe ? "has " + maybe : "no value");
  parts.push(String(!!text && n > 1));
  return parts.join(", ");
}

export function run(): void {
  console.log(label(""), label("Report"));
  console.log(count(0), count(3), count(NaN), count(-0), count(0.5));
  console.log(describe(null));
  console.log(describe({ name: "Ada" }));
  console.log(describe({ name: "Ada", nick: "" }));
  console.log(describe({ name: "Ada", nick: "Countess" }));
  console.log(flags("", 0, [], null));
  console.log(flags("x", 2, ["a"], "v"));
  const n: number | null = null;
  const value = n || 42;
  console.log(value, 0 || 7, "" || "fallback");
}
