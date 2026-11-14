// The TSX counter, compiled to Rust at build time (`docs/tsx.md`).
// Run it with `cargo run -p zant-tsx-counter` (WP-5.10 port of
// `examples/tsx/counter.tsx`; same component, keys and window).
import { useState, View, Row, Text, Button, Input, Switch, Checkbox } from "zant";
import type { u32 } from "zant";

export default function Counter() {
  const [count, setCount] = useState<u32>(0);
  const [step, setStep] = useState<u32>(1);
  const [name, setName] = useState("Zant");
  const [large, setLarge] = useState(false);
  const [notify, setNotify] = useState(true);

  const doubled = count * 2;

  function increment() {
    setCount(count + step);
  }

  return (
    <View padding={24} gap={14} alignItems="flex-start">
      <Text key="title" fontSize={large ? 28 : 20} fontWeight="semibold">
        {`Hello, ${name}!`}
      </Text>
      <Text key="count">
        Count: {count} (×2 = {doubled})
      </Text>
      <Input key="name" value={name} placeholder="Your name" width={240} onChange={setName} />
      <Row gap={8} alignItems="center">
        <Button key="inc" variant="primary" onClick={increment}>
          +{step}
        </Button>
        <Button key="reset" disabled={count === 0} onClick={() => setCount(0)}>
          Reset
        </Button>
        <Button key="step" onClick={() => setStep((s) => (s === 1 ? 5 : 1))}>
          Step {step}
        </Button>
      </Row>
      <Switch key="large" label="Large title" checked={large} onChange={setLarge} />
      <Checkbox key="notify" checked={notify} onChange={(on) => setNotify(on)}>
        Notify me
      </Checkbox>
      {notify && <Text key="note">Notifications are on</Text>}
      {count >= 10 ? <Text key="many">That is a lot of clicks.</Text> : null}
    </View>
  );
}
