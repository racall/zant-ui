import { useState, View, Row, Text, Link, Button, Icon, Tag, Clickable, Modal, Card, PdfPreview } from "zant";

interface CounterProps {
  label: string;
}

// stateful: in a list it needs a key
function Counter({ label }: CounterProps) {
  const [n, setN] = useState(0);
  return (
    <Button key={`count-${label}`} onClick={() => setN(n + 1)}>
      {label}: {n}
    </Button>
  );
}

export default function App() {
  const [open, setOpen] = useState(false);
  const [clicks, setClicks] = useState(0);
  const [cards, setCards] = useState(0);
  const [page, setPage] = useState(0);
  const names = ["a", "b"];

  return (
    <View padding={16} gap={8}>
      {/* nested spans and an inline link: one rich text */}
      <Text key="intro" size={16}>
        Read <Text weight="bold">this</Text> and <Text italic color="#cf1322">that</Text>, or{" "}
        <Link onClick={() => setClicks(clicks + 1)}>open the docs</Link>
        {clicks > 0 && ` (${clicks})`}.
      </Text>
      {/* an icon inside a button label; text labels on Tag and Link */}
      <Row gap={8}>
        <Button key="save" variant="primary" onClick={() => setOpen(true)}>
          <Icon name="check" /> Save
        </Button>
        <Tag>new</Tag>
        <Link key="more" onClick={() => setClicks(clicks + 1)}>
          More
        </Link>
      </Row>
      {/* a clickable card with its own button: the innermost element gets the click */}
      <Clickable key="card" onClick={() => setCards(cards + 1)}>
        <Card title={`Card clicked ${cards} times`}>
          <Button key="inner" onClick={() => setClicks(clicks + 10)}>
            Inner
          </Button>
        </Card>
      </Clickable>
      {names.map((name) => (
        <Counter key={name} label={name} />
      ))}
      {/* a TS number for an integer prop */}
      <PdfPreview src="missing.pdf" page={page + 0.5} height={80} />
      <Text key="page">page {page}</Text>
      {/* written in a row, rendered over the whole window */}
      <Row>{open && <Modal key="dialog" title="Saved" content={<Text>Done</Text>} onClose={() => setOpen(false)} />}</Row>
    </View>
  );
}
