// JSX spread attributes on tags and components.
import { useState, View, Row, Text, Button } from "zant";

interface ButtonLook {
  variant: "primary" | "default";
  size: "small" | "middle";
}

interface CardProps {
  title: string;
  count: number;
}

function Card({ title, count }: CardProps) {
  return (
    <Text key={title}>
      {title}: {count}
    </Text>
  );
}

export default function App() {
  const [n, setN] = useState(0);
  const look: ButtonLook = { variant: "primary", size: "small" };
  const card: CardProps = { title: "Clicks", count: n };
  return (
    <View padding={16} gap={8}>
      <Row gap={8}>
        <Button key="a" {...look} onClick={() => setN(n + 1)}>
          A
        </Button>
        {/* a later attribute wins; an object literal spreads its properties */}
        <Button key="b" {...look} variant="default" {...{ onClick: () => setN(0) }}>
          Reset
        </Button>
      </Row>
      <Card {...card} />
      <Card {...card} title="Override" />
    </View>
  );
}
