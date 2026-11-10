import { createStore, useStore, useEffect, View, Row, Text, Button, Switch } from "zant";
import type { u32 } from "zant";

interface Settings {
  dark: boolean;
  fontSize: u32;
  user: string;
  recent: string[];
}

// shared by every component (and other files that import it)
export const settings = createStore<Settings>({ dark: false, fontSize: 14, user: "", recent: [] });

// reads one field with a selector; changes it from a handler
function ThemeToggle() {
  const dark = useStore(settings, (s) => s.dark);
  return <Switch key="dark" checked={dark} onChange={(on) => settings.update({ dark: on })} />;
}

function FontSize() {
  const size = useStore(settings, (s) => s.fontSize);
  return (
    <Row gap={8} alignItems="center">
      <Button key="smaller" onClick={() => settings.update({ fontSize: size - 1 })}>
        A-
      </Button>
      <Text key="size">{size}px</Text>
      <Button key="bigger" onClick={() => settings.update({ fontSize: settings.get().fontSize + 1 })}>
        A+
      </Button>
    </Row>
  );
}

export default function App() {
  const s = useStore(settings);
  useEffect(() => {
    settings.update({ user: "guest" });
  }, []);

  function remember(word: string) {
    settings.set({ ...settings.get(), recent: [...settings.get().recent, word] });
  }

  return (
    <View padding={16} gap={8}>
      <Text key="summary">
        {s.user}: {s.dark ? "dark" : "light"}, {s.fontSize}px, {s.recent.length} recent
      </Text>
      <ThemeToggle />
      <FontSize />
      <Button key="remember" onClick={() => remember(`size ${s.fontSize}`)}>
        Remember
      </Button>
    </View>
  );
}
