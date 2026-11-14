import { useState, View, Text, Button } from "zant";
import statusIcon from "../assets/icons/statusbar.png";

// The fixture's root component: state, an event handler and an asset import
// (`_assets/` copy + `include_bytes!` in the generated module).
export default function App() {
  const [count, setCount] = useState(0);
  return (
    <View gap={8} padding={16}>
      <Text>{`clicks: ${count} (icon ${statusIcon.length} bytes)`}</Text>
      <Button onClick={() => setCount(count + 1)}>add</Button>
    </View>
  );
}
