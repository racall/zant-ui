// RegExp: printed output must match Node.

function words(text: string): string[] {
  const found: string[] = [];
  const re = /\b(\w)(\w*)\b/g;
  let m = re.exec(text);
  while (m !== null) {
    found.push(m[1].toUpperCase() + m[2] + "@" + m.index + "/" + re.lastIndex);
    m = re.exec(text);
  }
  return found;
}

function dates(text: string): string {
  const m = text.match(/(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})/);
  if (!m) {
    return "no date";
  }
  const year = m.groups?.year ?? "?";
  return `${year}/${m[2]}/${m[3]} at ${m.index} of ${m.length}`;
}

function compile(pattern: string): string {
  try {
    const re = new RegExp(pattern, "u");
    return "ok " + re.source + " " + re.flags + " " + re.unicode;
  } catch (e) {
    return e instanceof SyntaxError ? "SyntaxError: " + e.message : "other";
  }
}

export function run(): void {
  console.log(/^\d+$/.test("12345"), /^\d+$/.test("12a45"));
  console.log(words("hello big world").join(" | "));
  console.log(dates("released 2026-11-16, patched 2026-12-01"));
  console.log(dates("nothing"));
  const all = "a1b22c333".match(/\d+/g);
  console.log(all ? all.join(",") + " " + all.length : "none");
  const one = "a1b22c333".match(/(\d)(\d)/);
  console.log(one ? one[0] + " " + one[1] + " " + one.index : "none");
  for (const item of "k1=v1;k2=v2;k3=".matchAll(/(\w+)=(\w*)/g)) {
    console.log(item[1], "->", item[2] === "" ? "(empty)" : item[2], item.index);
  }
  console.log("John Smith".replace(/(\w+)\s(\w+)/, "$2, $1"));
  console.log("aaa".replace(/a/g, "[$&]"), "x-y".replace(/-/, "$`|$'"), "cost".replace(/o/, "$$"));
  console.log("2026-11-16".replace(/(?<y>\d+)-(?<m>\d+)-(?<d>\d+)/, "$<d>.$<m>.$<y>"));
  console.log("hello world".replace(/\b\w/g, (c) => c.toUpperCase()));
  console.log("price: 10, 20".replaceAll(/(\d+)/g, (m, n) => String(Number(n) * 2)));
  console.log("a, b,c ,  d".split(/\s*,\s*/).join("|"));
  console.log("a1b2c3".split(/(\d)/).join("|"), "a1b2c3".split(/\d/, 2).join("|"));
  console.log("".split(/,/).length, "abc".split(/(?:)/).join("|"));
  console.log("中文 emoji 🎉 test".search(/test/), "中文 emoji 🎉 test".search(/🎉/u));
  console.log("ÄBC äbc".match(/äbc/gi)?.join(",") ?? "none");
  const sticky = /foo/y;
  sticky.lastIndex = 3;
  console.log(sticky.test("barfoo"), sticky.lastIndex, sticky.test("barfoo"), sticky.lastIndex);
  console.log(compile("a+(b|c)"), "/", compile("(unclosed"));
  console.log(String(/a\/b/gim), /x/g.global, /x/.ignoreCase, /x/s.dotAll);
  console.log("one two  three".split(" ").length, "tab\tsep".replace(/\t/, "->"));
}
