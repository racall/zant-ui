// Desktop hooks: tray, global hotkey, clipboard changes, Dock reopen.
import { useState, useTray, useHotkey, useClipboardChange, useReopen, View, Text } from "zant";

// PNG bytes in a real app come from an imported .png or a native module
const icon: Uint8Array = [137, 80, 78, 71];

export default function App() {
  const [sync, setSync] = useState(true);
  const [hotkey, setHotkey] = useState("ctrl+cmd+v");
  const [clips, setClips] = useState(0);
  const label = sync ? "Pause sync" : "Resume sync";
  const shown = useTray({
    icon: icon,
    template: true,
    tooltip: "Clips",
    items: [
      { title: "Open", onSelect: () => setClips(0) },
      { separator: true },
      { title: label, checked: sync, onSelect: () => setSync(!sync) },
    ],
    onClick: () => setClips(clips + 100),
  });
  const ok = useHotkey(hotkey, () => setClips(clips + 1));
  useClipboardChange(() => setClips(clips + 1));
  useReopen(() => setHotkey(""));
  return (
    <View padding={16}>
      <Text key="t">
        {clips} {ok ? "hotkey" : "none"} {shown ? "tray" : "no tray"}
      </Text>
    </View>
  );
}
