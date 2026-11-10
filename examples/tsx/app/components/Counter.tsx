import { useState, Row, Text, Button } from "zant";

export interface CounterProps {
  label: string;
  onChange: (delta: number) => void;
}

// a child component with its own state
export function Counter({ label, onChange }: CounterProps) {
  const [n, setN] = useState(0);
  function add(delta: number) {
    setN(n + delta);
    onChange(delta);
  }
  return (
    <Row gap={8} alignItems="center">
      <Text key={`label-${label}`}>
        {label}: {n}
      </Text>
      <Button key={`minus-${label}`} onClick={() => add(-1)}>
        −
      </Button>
      <Button key={`plus-${label}`} variant="primary" onClick={() => add(1)}>
        +
      </Button>
    </Row>
  );
}
