// A TS + TSX app as `zant create` makes it: no Zig to write. The build
// (`addTsxApp`) compiles every .tsx here and generates the entry from this
// file's default export and app_config.zig. Run it with `zig build run-tsx-app`.
import { createStore, createWindow, useState, useStore, useKeyDown, View, Row, Heading, Paragraph, Button } from "zant";
import { clipboard, keyLabel, useTray, useAppExit, app, mainWindow, isMacos } from "zant";
import type { KeyEvent } from "zant";
import { Counter } from "./components/Counter";
import { Panel } from "./components/Panel";
// tray / menu bar icons: macOS draws a template image, Windows picks by taskbar theme
import statusbar from "./assets/statusbar.png";
import trayDark from "./assets/tray-dark.png";
import trayLight from "./assets/tray-light.png";

export interface Counts {
  total: number;
}

// shared by both windows
export const counts = createStore<Counts>({ total: 0 });

// a floating panel window, opened on first use
export const panel = createWindow(Panel, { title: "Panel", style: "panel", width: 260, height: 220, resizable: false });

export default function App() {
  const total = useStore(counts, (c) => c.total);
  const [copied, setCopied] = useState(false);
  useTray({
    icon: isMacos ? statusbar : trayDark,
    iconLight: trayLight,
    template: isMacos,
    tooltip: `TSX App · total ${total}`,
    items: [
      { title: "Show window", onSelect: () => mainWindow.show() },
      { title: "Toggle panel", onSelect: () => panel.toggle() },
      { separator: true },
      { title: "Quit", onSelect: () => app.quit() },
    ],
    onClick: () => mainWindow.show(),
  });
  // the last chance to stop native workers and save (a real app: session.stop())
  useAppExit(() => console.log(`TSX App: exiting with total ${counts.get().total}`));
  useKeyDown((e: KeyEvent) => {
    if (!e.mod || e.key !== "p") return false;
    panel.toggle();
    return true;
  });
  const copy = () => {
    if (!clipboard.setText(`Total: ${total}`)) return;
    setCopied(true);
    setTimeout(() => setCopied(false), 1500); // hide the note again
  };
  return (
    <View padding={24} gap={14}>
      <Heading level={2}>Hello from TSX</Heading>
      <Paragraph>Edit src/App.tsx and src/components/ to change this window.</Paragraph>
      <Counter label="Apples" onChange={(d) => counts.update({ total: total + d })} />
      <Counter label="Pears" onChange={(d) => counts.update({ total: total + d })} />
      <Row gap={8} alignItems="center">
        <Paragraph key="total">Total: {total}</Paragraph>
        <Button key="toggle-panel" onClick={() => panel.toggle()}>
          Toggle panel
        </Button>
        <Button key="copy" onClick={copy}>
          Copy total
        </Button>
      </Row>
      <Paragraph key="note">{copied ? "Copied to the clipboard" : `Panel: ${keyLabel("mod+p")}`}</Paragraph>
    </View>
  );
}
