import { useState, View, Input, Text } from "zant";

export default function App() {
  const [query, setQuery] = useState("");
  const titles = ["Alpha 中😀", "Beta", "Alphabet"];
  const visible = titles.filter((title) => title.toLowerCase().includes(query.trim().toLowerCase()));
  const i = query.indexOf("a");
  const last = "中😀abc😀".lastIndexOf("😀");
  const optional: string | null = "中😀".at(-1);
  return (
    <View padding={16} gap={4}>
      <Input key="search" value={query} onChange={setQuery} />
      <Text key="results">{visible.join(" | ")}</Text>
      <Text key="index">{i >= 0 ? "found" : "missing"}: {i} / {last}</Text>
      <Text key="checks">{query.startsWith("a") ? "prefix" : "no"}/{query.endsWith("a") ? "suffix" : "no"}</Text>
      <Text key="case">{query.trimStart().trimEnd().toUpperCase()}</Text>
      <Text key="slice">{"A中😀Z".slice(1, -1)} / {"abcd".substring(3, 1)}</Text>
      <Text key="defaults">{"abc".slice(1)} / {"abc".substring(1)}</Text>
      <Text key="padding">{"x".padStart(3)} / {"x".padEnd(4, "ab")}</Text>
      <Text key="repeat">{"中".repeat(2)}</Text>
      <Text key="replace">{"a-a".replace("a", "b")} / {"a-a".replaceAll("a", "b")}</Text>
      <Text key="split">{"中,😀".split(",").join("-")}</Text>
      <Text key="char">{"A中".charAt(1)} / {optional ?? "none"} / {"".at(0) ?? "none"}</Text>
    </View>
  );
}
