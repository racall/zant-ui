// App hooks of the root component: close request and global keys.
import { useState, useCloseRequest, useKeyDown, View, Text } from "zant";
import type { KeyEvent } from "zant";

export default function App() {
  const [dirty, setDirty] = useState(true);
  const [asked, setAsked] = useState(0);
  const [last, setLast] = useState("");
  useCloseRequest(() => {
    if (dirty) {
      setAsked(asked + 1);
      return false;
    }
    return true;
  });
  useKeyDown((e: KeyEvent) => {
    setLast(e.key);
    if (e.key === "Escape") {
      setDirty(false);
      return true;
    }
    return e.mod && e.key === "s";
  });
  return (
    <View padding={16}>
      <Text key="state">
        {dirty ? "dirty" : "clean"} {asked} {last}
      </Text>
    </View>
  );
}
