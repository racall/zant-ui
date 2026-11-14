// A second window (`createWindow` in App.tsx): a floating panel that shares a
// store with the main window and hides itself on Esc.
import { useState, useStore, useKeyDown, View, Text, Button } from "zant";
import type { KeyEvent } from "zant";
import { counts, panel } from "../App";

export function Panel() {
  const total = useStore(counts, (c) => c.total);
  const [opened, setOpened] = useState(0);
  useKeyDown((e: KeyEvent) => {
    if (e.key === "Escape") {
      panel.hide();
      return true;
    }
    return false;
  });
  return (
    <View padding={16} gap={10} grow={1}>
      <Text key="panel-total" size={16} weight="semibold">
        Total: {total}
      </Text>
      <Text key="panel-opened">Clicked here {opened} times</Text>
      <Button key="panel-more" variant="primary" onClick={() => setOpened(opened + 1)}>
        Click
      </Button>
      <Button key="panel-reset" onClick={() => counts.set({ total: 0 })}>
        Reset total
      </Button>
    </View>
  );
}
