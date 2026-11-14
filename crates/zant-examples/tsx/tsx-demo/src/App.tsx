// A TS + TSX app as `zant create` makes it: no Rust to write (WP-5.10 port of
// `examples/tsx/app/App.tsx`). `build.rs` compiles every .tsx here, the
// runtime entry is `src/main.rs`, and the UI is this file plus `components/`.
// Run it with `cargo run -p zant-tsx-demo`.
import { createStore, useState, useStore, useKeyDown, View, Row, Heading, Paragraph, Button } from "zant";
import { clipboard, keyLabel, useTray, useAppExit, app, mainWindow, isMacos } from "zant";
import type { KeyEvent } from "zant";
import { Counter } from "./components/Counter";
import { panel } from "./components/Panel";
// the app's native module (`native/system.ts` → `native/system.rs`): TSX calls
// the Rust function directly, no bridge and no serialization
import { info } from "native/system";
// tray / menu bar icons: macOS draws a template image, Windows picks by taskbar theme
import statusbar from "../assets/icons/statusbar.png";
import trayDark from "../assets/icons/tray-dark.png";
import trayLight from "../assets/icons/tray-light.png";

export interface Counts {
  total: number;
}

// shared by both windows
export const counts = createStore<Counts>({ total: 0 });

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
  // the app's native module (`native/system.ts` → `native/system.rs`) is
  // called here: a TSX call compiles to a direct Rust call, no bridge
  useAppExit(() => console.log(`TSX App: exiting with total ${counts.get().total} (${info().cpus} cpus)`));
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
