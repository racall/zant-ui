// In-memory images and the hotkey recorder (tests/tsx_media_test.zig).
import { useState, View, Image, HotkeyInput, Text } from "zant";
import { thumbnail } from "./thumbs.zig";

export default function App() {
  const [hotkey, setHotkey] = useState("ctrl+cmd+v");
  const [recording, setRecording] = useState(false);
  const thumb = thumbnail(7);
  return (
    <View padding={16} gap={8}>
      <Image key="thumb" image={thumb} width={40} height={40} />
      <HotkeyInput
        key="hk"
        value={hotkey}
        recording={recording}
        onStart={() => setRecording(true)}
        onRecord={(s) => {
          setHotkey(s);
          setRecording(false);
        }}
        onCancel={() => setRecording(false)}
        onClear={() => setHotkey("")}
      />
      <Text key="label">{hotkey}</Text>
    </View>
  );
}
