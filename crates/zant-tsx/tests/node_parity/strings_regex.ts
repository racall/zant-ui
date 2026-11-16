function scan(text: string): string {
  const re = /(?<letter>[a-z])(\d*)/gi;
  const parts: string[] = [];
  let m = re.exec(text);
  while (m !== null) {
    parts.push(`${m[0]}:${m[1]}:${m[2]}:${m.index}:${re.lastIndex}:${m.groups?.letter ?? "missing"}`);
    m = re.exec(text);
  }
  return parts.join("|") + "/reset=" + re.lastIndex;
}

export function run(): void {
  console.log("flags", /a/gimsuy.flags, /a/g.global, /a/i.ignoreCase, /a/m.multiline, /a/s.dotAll, /a/u.unicode, /a/y.sticky);
  console.log("flags-false", /a/.global, /a/.ignoreCase, /a/.multiline, /a/.dotAll, /a/.unicode, /a/.sticky);
  console.log("source", /(?:)/.source, /a\/b/.source, new RegExp("a.b", "si").flags);
  console.log("source-escaped", new RegExp("\n\r\u2028\u2029/").source, new RegExp("").source);
  console.log("indices-flag", /a/d.hasIndices, /a/.hasIndices);
  console.log("multiline", /^b/m.test("a\nb\nc"), /^b/.test("a\nb\nc"));
  console.log("dotAll", /^a.b$/s.test("a\nb"), /^a.b$/.test("a\nb"));
  console.log("unicode", /^.$/u.test("😀"), /^..$/.test("😀"));
  console.log("ignoreCase", /ä/iu.test("Ä"), /k/iu.test("K"), /s/iu.test("ſ"));
  console.log("word-fold", /\w/iu.test("K"), /\w/iu.test("ſ"), /\bK\b/iu.test("K"));
  console.log("lookaround", /(?<=中)a(?=b)/u.test("中ab"), /(a)\1/.test("aa"));
  console.log("unicode-property", /\p{Letter}+/u.test("中文"), /\p{Emoji}/u.test("😀"));
  console.log("exec-loop", scan("😀A12 b C3"));
  const sticky = /a/y;
  sticky.lastIndex = 1;
  console.log("sticky-hit", sticky.test("baab"), sticky.lastIndex);
  console.log("sticky-next", sticky.test("baab"), sticky.lastIndex);
  console.log("sticky-fail", sticky.test("baab"), sticky.lastIndex);
  const global = /a/g;
  global.lastIndex = -3;
  console.log("negative-lastIndex", global.test("aba"), global.lastIndex);
  global.lastIndex = 1.9;
  console.log("fractional-lastIndex", global.test("aba"), global.lastIndex);
  global.lastIndex = 99;
  console.log("oob-lastIndex", global.test("aba"), global.lastIndex);
  global.lastIndex = NaN;
  console.log("nan-lastIndex", global.test("aba"), global.lastIndex);
  global.lastIndex = Infinity;
  console.log("infinite-lastIndex", global.test("aba"), global.lastIndex);
  const astral = /./uy;
  astral.lastIndex = 1;
  const astralMatch = astral.exec("😀x");
  console.log("inside-surrogate", astralMatch ? astralMatch[0] : "missing", astralMatch ? astralMatch.index : -1, astral.lastIndex);
  const plain = /b/;
  plain.lastIndex = 99;
  console.log("plain-lastIndex", plain.test("abc"), plain.lastIndex);
  const search = /b/gy;
  search.lastIndex = 2;
  console.log("search-sticky", "abc".search(search), search.lastIndex);
  const searchGlobal = /b/g;
  searchGlobal.lastIndex = 2;
  console.log("search-global", "😀abc".search(searchGlobal), searchGlobal.lastIndex);
  console.log("search-empty", "😀".search(/$/u), "".search(/x/), "".search(/(?:)/));
  const empty = /(?:)/gu;
  const parts: string[] = [];
  let m = empty.exec("😀x");
  while (m !== null) {
    parts.push(`${m.index}:${empty.lastIndex}`);
    if (m.index === 0) {
      empty.lastIndex = 2;
    } else {
      empty.lastIndex = m.index + 1;
    }
    m = empty.exec("😀x");
  }
  console.log("empty-exec", parts.join("|"), empty.lastIndex);
  const all = /(?<word>[a-z]+)/g;
  all.lastIndex = 2;
  for (const item of "a bb c".matchAll(all)) {
    console.log("matchAll", item[0], item.index, item.groups?.word ?? "missing", all.lastIndex);
  }
  console.log("matchAll-preserves", all.lastIndex);
  for (const item of "😀x".matchAll(/(?:)/gu)) {
    console.log("matchAll-empty", item.index, item[0].length);
  }
  const matched = "a1b22".match(/\d+/g);
  console.log("match-global", matched ? matched.join("|") : "missing");
  const named = "😀x12".match(/(?<letter>x)(\d+)/u);
  console.log("match-named", named ? `${named.index}:${named.length}:${named.slice(1).join("|")}:${named.groups?.letter ?? "missing"}` : "missing");
  const reuse = /a/g;
  reuse.lastIndex = 2;
  console.log("match-resets", "aba".match(reuse)?.join("|") ?? "missing", reuse.lastIndex);
  reuse.lastIndex = 2;
  console.log("replace-resets", "aba".replace(reuse, "x"), reuse.lastIndex);
}
