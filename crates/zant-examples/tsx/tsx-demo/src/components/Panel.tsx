// A second window: a floating panel that shares a store with the main window
// and hides itself on Esc (same component and options as
// `examples/tsx/app`; see `src/App.tsx` for the `panel` handle it imports).
import { useState, useStore, useKeyDown, createWindow, View, Text, Button } from "zant";
import type { KeyEvent } from "zant";
import { counts } from "../App";

function Panel() {
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

// The window is declared in the component's own file: `crates/zant-tsx` emits
// `impl tsx::window::Component` in the file a window's component is defined in,
// and the handle is imported from here (`docs/tsx.md`).
export const panel = createWindow(Panel, { title: "Panel", style: "panel", width: 260, height: 220, resizable: false });
