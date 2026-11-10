// Optionals as conditions, narrowed in the branch they guard (TS control flow).
import { useState, View, Row, Text, Button } from "zant";

interface Peer {
  name: string;
  files: string[];
}

interface Session {
  peer: Peer | null;
}

/// Member paths narrow too; a string iterates by code point.
function describe(s: Session): string {
  if (s.peer === null) return "alone";
  let initials = "";
  for (const ch of s.peer.name) {
    if (ch.charCodeAt(0) >= 97) initials += ch.toUpperCase();
  }
  return s.peer.files.length > 0 ? `${initials}+files` : initials;
}

function PeerCard({ peer, note }: { peer: Peer | null; note?: string }) {
  if (!peer) return <Text>No peer</Text>;
  const count = peer.files.length;
  return (
    <View gap={4}>
      <Text key="name">
        {peer.name} ({count})
      </Text>
      {note && <Text key="note">{note}</Text>}
    </View>
  );
}

export default function App() {
  const [peer, setPeer] = useState<Peer | null>(null);
  const [label, setLabel] = useState<string | null>(null);
  const [picked, setPicked] = useState("");
  const current = peer;
  const title = label ? label : "untitled";
  const size = current !== null ? current.files.length : 0;
  const empty = !current || current.files.length == 0;
  const connect = () => {
    if (peer == null) return;
    setLabel(peer.name);
  };
  return (
    <View padding={16} gap={8}>
      <Text key="title">
        {title} {size} {empty ? "empty" : "has files"}
      </Text>
      <PeerCard key="card" peer={peer} note={label} />
      {current && (
        <Row key="files" gap={4}>
          {current.files.map((f) => (
            <Button key={f} onClick={() => setPicked(`${current.name}/${f}`)}>
              {f}
            </Button>
          ))}
        </Row>
      )}
      <Button key="add" onClick={() => setPeer({ name: "mac", files: ["a.txt", "b.png"] })}>
        Add peer
      </Button>
      <Button key="connect" onClick={connect}>
        Connect
      </Button>
      <Text key="picked">{picked}</Text>
      <Text key="describe">{describe({ peer: peer })}</Text>
    </View>
  );
}
