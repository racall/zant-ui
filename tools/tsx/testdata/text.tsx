import { useState, View, Text, Button, Checkbox, Switch } from "zant";

export default function TextExamples(): React.ReactNode {
  const [name, setName] = useState("Zant");
  const [count, setCount] = useState<u32>(3);
  const [enabled, setEnabled] = useState(false);
  const greeting = "Hello, " + name + "!";
  const length = name.length;
  const message = `Braces {ok} and UTF-8 你好: ${name} ${count} ${enabled}`;
  return (
    <View>
      <Text key="whitespace" fontWeight={600} color="#123" fontSize={18}
        textAlign="center" wrap numberOfLines={2} lineHeight={24} fontFamily="sans">
        Hello &amp; world
        &lt;tags&gt; &#x4f60;&#22909; &quot;quotes&quot;
      </Text>
      <Text>{greeting} {length} {message} {1.25} {true}</Text>
      <Text fontWeight="bold" family="serif">{`Static {braces}`}</Text>
      <Text weight="400" family="mono">{"Escapes: \" \\ \n"}</Text>
      <Text>{name === "Zant" ? "same" : "different"} {name !== "other"}</Text>
      <Button onClick={() => setCount(count + 1)}>Count: {count}</Button>
      <Button text="Empty child list" />
      <Checkbox checked={enabled} onChange={setEnabled}>Enabled: {enabled}</Checkbox>
      <Switch checked={enabled} onChange={() => setEnabled(!enabled)}>Toggle {name}</Switch>
      <Text />
    </View>
  );
}
