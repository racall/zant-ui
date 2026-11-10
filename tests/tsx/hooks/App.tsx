// Custom hooks (docs/tsx.md): in the root and in a child component.
import { Column, Row, Text, Button } from "zant";
import { useCounter } from "./hooks/useCounter";
import { useToggle } from "./hooks/useToggle";

interface ClickerProps {
  label: string;
}

function Clicker({ label }: ClickerProps) {
  const { count, increment } = useCounter(100);
  return (
    <Row>
      <Text key={`clicker-text-${label}`}>{`${label} ${count}`}</Text>
      <Button key={`clicker-${label}`} onClick={increment}>+</Button>
    </Row>
  );
}

export default function App() {
  const { count, increment, reset } = useCounter(2);
  const other = useCounter(10);
  const [on, toggle, flips] = useToggle();
  return (
    <Column>
      <Text key="count">{`${count}`}</Text>
      <Text key="other">{`${other.count}`}</Text>
      <Text key="on">{`${on ? "on" : "off"} ${flips}`}</Text>
      <Button key="inc" onClick={increment}>+</Button>
      <Button key="inc-other" onClick={other.increment}>+</Button>
      <Button key="reset" onClick={reset}>0</Button>
      <Button key="toggle" onClick={toggle}>t</Button>
      <Clicker key="a" label="a" />
      <Clicker key="b" label="b" />
    </Column>
  );
}
