// Default values: parameters callers may leave out, optional props with defaults.
import { useState, View, Row, Text, Button } from "zant";

const step = 5;

function label(n: number, unit = "item", plural: boolean = true): string {
  return n == 1 || !plural ? `${n} ${unit}` : `${n} ${unit}s`;
}

interface BadgeProps {
  text: string;
  size?: number;
  tone?: "info" | "warn";
  bold?: boolean;
}

function Badge({ text, size = 14, tone = "info", bold = false }: BadgeProps) {
  return (
    <Text size={size} weight={tone === "warn" || bold ? "semibold" : "regular"}>
      {text}
    </Text>
  );
}

export default function App() {
  const [count, setCount] = useState(0);
  function add(by = step) {
    setCount(count + by);
  }
  return (
    <View padding={16} gap={8}>
      <Badge key="count" text={label(count)} bold />
      <Badge key="files" text={label(count, "file", false)} size={20} tone="warn" />
      <Row key="buttons" gap={8}>
        <Button key="one" onClick={() => add(1)}>
          +1
        </Button>
        <Button key="step" onClick={() => add()}>
          +{step}
        </Button>
      </Row>
    </View>
  );
}
