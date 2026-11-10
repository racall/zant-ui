// Named handlers that read render locals (React's `const onX = () => …`):
// each place they are passed captures the values of that render.
import { useState, useHotkey, View, Row, Text, Button, Input } from "zant";

function Stepper({ label, onStep }: { label: string; onStep: (delta: number) => void }) {
  return (
    <Row gap={8}>
      <Text key="label">{label}</Text>
      <Button key="plus" onClick={() => onStep(1)}>+</Button>
    </Row>
  );
}

export default function App() {
  const [count, setCount] = useState(0);
  const [name, setName] = useState("");
  const [said, setSaid] = useState("");
  const step = name.length > 0 ? name.length : 1;
  const greeting = `Hello ${name}`;
  const bump = () => setCount(count + step);
  const onStep = (delta: number) => setCount(count + delta * step);
  const announce = () => {
    setSaid(greeting);
    bump();
  };
  useHotkey("ctrl+alt+b", bump);
  return (
    <View padding={16} gap={8}>
      <Input key="name" value={name} onChange={setName} />
      <Text key="count">
        {count} (step {step})
      </Text>
      <Text key="said">{said}</Text>
      <Row key="buttons" gap={8}>
        <Button key="bump" onClick={bump}>Bump</Button>
        <Button key="again" onClick={bump}>Bump again</Button>
        <Button key="announce" onClick={announce}>Announce</Button>
      </Row>
      <Stepper key="stepper" label={greeting} onStep={onStep} />
    </View>
  );
}
