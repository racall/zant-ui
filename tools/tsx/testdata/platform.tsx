// Desktop functions from "zant": clipboard, the app, the main window, platform info.
import { useState, clipboard, app, mainWindow, wakeAfter, keyLabel, isMacos, titlebarInsets } from "zant";
import { View, Row, Text, Button, Input } from "zant";

export default function App() {
  const [draft, setDraft] = useState("");
  const [copied, setCopied] = useState(0);
  const [until, setUntil] = useState(0);
  const insets = titlebarInsets();
  const now = performance.now();
  const toast = now < until;
  if (toast) wakeAfter(until - now);
  const hint = `Search ${keyLabel("mod+f")}`;
  const copy = () => {
    if (clipboard.setText(draft)) {
      setCopied(clipboard.changeCount());
      setUntil(performance.now() + 1800);
    }
  };
  const paste = () => {
    const files = clipboard.files();
    setDraft(files ?? clipboard.text());
  };
  return (
    <View padding={16} gap={8}>
      <Row key="bar" padding={{ left: insets.left, right: insets.right }}>
        <Text key="os">{isMacos ? "macOS" : "Windows"} · {app.deviceName()}</Text>
      </Row>
      <Input key="draft" value={draft} placeholder={hint} onChange={setDraft} />
      <Row key="actions" gap={8}>
        <Button key="copy" onClick={copy}>Copy</Button>
        <Button key="paste" onClick={paste}>Paste</Button>
        <Button key="hide" onClick={() => mainWindow.hide()}>Hide</Button>
        <Button key="login" onClick={() => app.setLaunchAtLogin(app.launchAtLogin() == "off")}>
          Login: {app.launchAtLogin()}
        </Button>
        <Button key="quit" onClick={() => app.quit()}>Quit</Button>
      </Row>
      {toast && <Text key="toast">Copied #{copied} · {app.dataDir() ?? "no data dir"}</Text>}
      {clipboard.image() != null && <Text key="image">The clipboard holds an image</Text>}
    </View>
  );
}
