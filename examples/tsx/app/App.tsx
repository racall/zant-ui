// A TS + TSX app as `zant create` makes it: no Zig to write. The build
// (`addTsxApp`) compiles every .tsx here and generates the entry from this
// file's default export and app_config.zig. Run it with `zig build run-tsx-app`.
import { createStore, createWindow, useStore, View, Row, Heading, Paragraph, Button } from "zant";
import { Counter } from "./components/Counter";
import { Panel } from "./components/Panel";

export interface Counts {
  total: number;
}

// shared by both windows
export const counts = createStore<Counts>({ total: 0 });

// a floating panel window, opened on first use
export const panel = createWindow(Panel, { title: "Panel", style: "panel", width: 260, height: 220, resizable: false });

export default function App() {
  const total = useStore(counts, (c) => c.total);
  return (
    <View padding={24} gap={14}>
      <Heading level={2}>Hello from TSX</Heading>
      <Paragraph>Edit src/App.tsx and src/components/ to change this window.</Paragraph>
      <Counter label="Apples" onChange={(d) => counts.update({ total: total + d })} />
      <Counter label="Pears" onChange={(d) => counts.update({ total: total + d })} />
      <Row gap={8} alignItems="center">
        <Paragraph key="total">Total: {total}</Paragraph>
        <Button key="toggle-panel" onClick={() => panel.toggle()}>
          Toggle panel
        </Button>
      </Row>
    </View>
  );
}
