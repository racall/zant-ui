import { View, Text, Button, useState } from "zant";

interface Item { title: string; id: number; }

function Label({ title: label }: { title: string }) {
  return <Text>{label}</Text>;
}

function DefaultLabel({ id, title: label = "Untitled" }: { id: number; title?: string }) {
  return <Text>{id}: {label}</Text>;
}

// A default is legal even when the property is required; callers still supply it.
function RequiredLabel({ title: label = "Untitled" }: { title: string }) {
  return <Text>{label}</Text>;
}

function StatefulLabel({ id, title: label }: Item) {
  const [count, setCount] = useState(0);
  function increment() { setCount(count + id); }
  return <Button onClick={increment}>{label}: {count}</Button>;
}

export default function App() {
  const item: Item = { title: "Hello", id: 7 };
  const { title: label, id } = item;
  return <View>
    <Label title={label} />
    <DefaultLabel id={id} />
    <DefaultLabel id={id} title="" />
    <RequiredLabel title={item.title} />
    <StatefulLabel id={id} title={label} />
  </View>;
}
