// A TS + TSX app as `zant create` makes it: no Zig to write. The build
// (`addTsxApp`) compiles every .tsx here and generates the entry from this
// file's default export and app_config.zig. Run it with `zig build run-tsx-app`.
import { useState, View, Heading, Paragraph } from "zant";
import { Counter } from "./components/Counter";

export default function App() {
  const [total, setTotal] = useState(0);
  return (
    <View padding={24} gap={14}>
      <Heading level={2}>Hello from TSX</Heading>
      <Paragraph>Edit src/App.tsx and src/components/ to change this window.</Paragraph>
      <Counter label="Apples" onChange={(d) => setTotal(total + d)} />
      <Counter label="Pears" onChange={(d) => setTotal(total + d)} />
      <Paragraph key="total">Total: {total}</Paragraph>
    </View>
  );
}
