// Another window: createWindow, its handle, and app hooks in its component.
import { useState, createWindow, useKeyDown, useWindowFocus, useCloseRequest, View, Text, Button } from "zant";
import type { KeyEvent } from "zant";

function Panel() {
  const [shown, setShown] = useState(0);
  useWindowFocus((focused: boolean) => {
    if (!focused) panel.hide();
  });
  useCloseRequest(() => {
    panel.hide();
    return false;
  });
  useKeyDown((e: KeyEvent) => {
    if (e.key === "Escape") {
      panel.hide();
      return true;
    }
    return false;
  });
  return (
    <View padding={12}>
      <Button key="count" onClick={() => setShown(shown + 1)}>
        Clicked {shown}
      </Button>
    </View>
  );
}

export const panel = createWindow(Panel, {
  title: "Panel",
  style: "panel",
  width: 380,
  height: 480,
  minWidth: 380,
  resizable: false,
  background: "#ffffff",
});

export default function App() {
  return (
    <View padding={16} gap={8}>
      <Text key="visible">{panel.visible() ? "open" : "closed"}</Text>
      <Button key="toggle" onClick={() => panel.toggle()}>
        Toggle
      </Button>
      <Button
        key="corner"
        onClick={() => {
          panel.place("trayCorner", 12);
          panel.activate();
          panel.focus("count");
        }}
      >
        By the tray
      </Button>
    </View>
  );
}
