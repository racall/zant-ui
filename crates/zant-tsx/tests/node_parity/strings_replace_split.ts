function show(label: string, value: string): void {
  console.log(label, JSON.stringify(value));
}

export function run(): void {
  show("string-tokens", "abcabc".replace("b", "[$1][$&][$`][$'][$$][$<x>]"));
  show("string-all-tokens", "abcabc".replaceAll("b", "[$1][$&][$`][$'][$$][$<x>]"));
  show("replace-empty", "abc".replace("", "-"));
  show("replace-all-empty", "abc".replaceAll("", "-"));
  show("replace-empty-input", "".replaceAll("", "-"));
  show("replace-missing", "abc".replaceAll("z", "$&"));
  show("replace-overlap", "aaaaa".replaceAll("aa", "x"));
  show("regex-tokens", "zabq".replace(/(a)(b)/, "[$1][$2][$&][$`][$'][$$][$<x>]"));
  show("regex-number-tokens", "ab".replace(/(a)(b)/, "$0/$01/$10/$12/$20/$99/$3"));
  show("regex-named", "x=12 y=34".replaceAll(/(?<key>\w)=(?<value>\d+)/g, "$<value>:$<key>:$<missing>:$$"));
  show("regex-named-unmatched", "b".replace(/(?<a>a)?b/, "[$<a>][$1]"));
  show("regex-dollar", "x".replace(/x/, "$$$&|$$$$|$<|$>|$"));
  show("regex-empty-global", "ab".replace(/(?:)/g, "-"));
  show("regex-empty-unicode", "😀".replaceAll(/(?:)/gu, "-"));
  show("replacer", "a1b22".replace(/([a-z])(\d+)/g, (m: string, a: string, n: string): string => a.toUpperCase() + String(Number(n) * 2)));
  show("replacer-literal-dollar", "aa".replaceAll(/a/g, (m: string): string => "$&$$"));
  show("split-empty-fields", "a,,b,".split(",").join("|"));
  show("split-empty-input", "".split(",").join("|"));
  console.log("split-empty-count", "".split(",").length, "".split("").length);
  show("split-empty-separator", "a中b".split("").join("|"));
  show("split-regex-limit", "a,b,c".split(/,/, 2).join("|"));
  console.log("split-zero", "ab".split(/b/, 0).length);
  show("split-negative-limit", "a,b,c".split(/,/, -1).join("|"));
  show("split-fractional-limit", "a,b,c".split(/,/, 2.9).join("|"));
  console.log("split-wrapped-limit", "a,b".split(/,/, 4294967296).length);
  console.log("split-infinite-limit", "a,b".split(/,/, Infinity).length);
  show("split-regex", " a , b, c ".split(/\s*,\s*/).join("|"));
  show("split-captures", "a1b22c".split(/(\d+)/).join("|"));
  show("split-capture-limit", "a1b22c".split(/(\d+)/, 4).join("|"));
  show("split-zero-width", "ab".split(/(?:)/).join("|"));
  show("split-lookahead", "abac".split(/(?=a)/).join("|"));
  show("split-unicode", "😀中😀".split(/(?:)/u).join("|"));
  show("split-anchors", "ab".split(/^|$/).join("|"));
  console.log("split-empty-regex", "".split(/(?:)/).length, "".split(/x/).length);
  const re = /,/gy;
  re.lastIndex = 2;
  show("split-lastIndex", "a,b,c".split(re).join("|"));
  console.log("split-preserves", re.lastIndex);
}
