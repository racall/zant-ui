import { useState, useTheme, List, View, Row, Text, Button } from "zant";
import type { u32 } from "zant";

interface Item {
  id: u32;
  title: string;
  done: boolean;
}

export default function Feed() {
  const theme = useTheme();
  const [items, setItems] = useState<Item[]>([
    { id: 1, title: "One", done: false },
    { id: 2, title: "Two", done: true },
  ]);
  const [picked, setPicked] = useState<u32>(0);
  const visible = items.filter((i) => !i.done || picked > 0);

  return (
    <View flex={1} padding={16} gap={8}>
      <Text>Picked {picked}</Text>
      <List
        key="feed"
        data={visible}
        estimatedItemHeight={40}
        gap={4}
        keyExtractor={(item) => item.id}
        renderItem={(item, index) => (
          <Row key={`row-${item.id}`} gap={8} background={index % 2 === 0 ? theme.colorFillQuaternary : theme.colorBgContainer}>
            <Text>{item.title}</Text>
            <Button key={`pick-${item.id}`} onClick={() => setPicked(item.id)}>
              Pick
            </Button>
          </Row>
        )}
      />
      <List key="names" data={["Ada", "Grace"]} keyExtractor={(name) => name} renderItem={(name) => <Text>{name}</Text>} />
    </View>
  );
}
