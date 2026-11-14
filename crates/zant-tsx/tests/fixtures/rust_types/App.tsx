// Rust structs and enums used from TSX.
import { useState, View, Text, Button } from "zant";
import { peers, label, firstOnline } from "./devices.rs";
import type { Peer, Kind } from "./devices.rs";

function icon(kind: Kind): string {
  return kind === "Laptop" ? "💻" : kind === "Desktop" ? "🖥" : "📱";
}

export default function Devices() {
  const [only, setOnly] = useState(false);
  const list: Peer[] = peers().filter((p) => !only || p.online);
  const first = firstOnline(list);
  return (
    <View padding={16} gap={8}>
      {list.map((p) => (
        <Text key={p.name}>
          {icon(p.kind)} {p.name} {label(p)} {p.tags.length} {p.last_seen ?? -1}
        </Text>
      ))}
      <Text key="first">{first?.name ?? "none"}</Text>
      <Button key="only" onClick={() => setOnly(!only)}>
        Online only
      </Button>
    </View>
  );
}
