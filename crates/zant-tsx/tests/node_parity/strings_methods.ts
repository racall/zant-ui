// Supported string methods; avoid the documented UTF-8 surrogate-boundary differences.
function show(label: string, value: string): void {
  console.log(label, JSON.stringify(value));
}

export function run(): void {
  const text = "A中😀B";
  console.log("utf16", text.length, text.indexOf("B"), text.lastIndexOf("😀"));
  console.log("find-empty", "".indexOf(""), "abc".lastIndexOf(""), "abc".indexOf("z"));
  console.log("predicates", "".includes(""), "".startsWith(""), "".endsWith(""));
  console.log("predicates-wide", text.includes("😀"), text.startsWith("A中"), text.endsWith("B"));
  console.log("codes", text.charCodeAt(0), text.charCodeAt(1), text.charCodeAt(2), text.charCodeAt(3));
  console.log("codepoints", text.codePointAt(2) ?? -1, text.codePointAt(3) ?? -1);
  console.log("codes-oob", "".charCodeAt(0), text.charCodeAt(-1), text.charCodeAt(99));
  console.log("points-oob", "".codePointAt(0) ?? -1, text.codePointAt(-1) ?? -1, text.codePointAt(99) ?? -1);
  console.log("fractional-code", "abc".charCodeAt(1.9), "abc".codePointAt(-0.9) ?? -1);
  console.log("at", "a中b".at(-1) ?? "missing", "a中b".at(-2) ?? "missing", "abc".at(1.9) ?? "missing");
  console.log("at-oob", "".at(0) ?? "missing", "abc".at(-4) ?? "missing", "abc".at(99) ?? "missing");
  console.log("at-special", "abc".at(NaN) ?? "missing", "abc".at(Infinity) ?? "missing", "abc".at(-0.9) ?? "missing");
  console.log("code-special", "abc".charCodeAt(NaN), "abc".charCodeAt(Infinity), "abc".codePointAt(NaN) ?? -1);
  show("char-oob", "abc".charAt(-1) + "/" + "abc".charAt(99) + "/" + "".charAt(0));
  show("slice-negative", "abcdef".slice(-4, -1));
  show("slice-swapped", "abcdef".slice(4, 1));
  show("slice-clamp", "abcdef".slice(-99, 99));
  show("slice-fraction", "abcdef".slice(1.9, 4.9));
  show("slice-infinity", "abcdef".slice(-Infinity, Infinity));
  show("slice-nan", "abcdef".slice(NaN, 3));
  show("substring-swapped", "abcdef".substring(4, 1));
  show("substring-negative", "abcdef".substring(4, -2));
  show("substring-clamp", "abcdef".substring(-99, 99));
  show("substring-fraction", "abcdef".substring(4.9, 1.9));
  show("substring-infinity", "abcdef".substring(Infinity, -Infinity));
  show("substring-nan", "abcdef".substring(NaN, 3));
  show("slice-wide", text.slice(1, 4));
  show("substring-wide", text.substring(4, 1));
  show("pad-start-long", "x".padStart(8, "abcdefghij"));
  show("pad-end-long", "x".padEnd(8, "abcdefghij"));
  show("pad-cycle", "xy".padStart(9, "abc") + "/" + "xy".padEnd(9, "abc"));
  show("pad-empty", "x".padStart(9, "") + "/" + "x".padEnd(9, ""));
  show("pad-default", "x".padStart(3) + "/" + "x".padEnd(3));
  show("pad-small", "abc".padStart(-1, "x") + "/" + "abc".padEnd(2, "x"));
  show("pad-fraction", "x".padStart(3.9, "ab") + "/" + "x".padEnd(NaN, "ab"));
  show("pad-wide", "中".padStart(5, "😀") + "/" + "中".padEnd(5, "😀"));
  show("repeat-zero", "abc".repeat(0));
  show("repeat-empty", "".repeat(4));
  show("repeat-fraction", "中".repeat(2.9));
  show("repeat-nan", "x".repeat(NaN));
  const spaces = "\u00a0\u1680\u2000\u2007\u2028\u2029\u202f\u205f\u3000\ufeff";
  show("trim", (spaces + "middle" + spaces).trim());
  show("trimStart", (spaces + "middle" + spaces).trimStart());
  show("trimEnd", (spaces + "middle" + spaces).trimEnd());
  show("trim-not-space", ("\u0085\u200b" + "x" + "\u200b\u0085").trim());
  show("ascii-case", "aZ09中".toLowerCase() + "/" + "aZ09中".toUpperCase());
  console.log("template", `${0.1 + 0.2}`, `${-0}`, `${1e21}`, `${1e-7}`, `${1e20}`);
  console.log("template-special", `${NaN}`, `${Infinity}`, `${-Infinity}`);
  console.log("truthy", "" || "fallback", "中" || "fallback", !!"", !!"😀", !!NaN, !!-0);
}
