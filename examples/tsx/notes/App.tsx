// A TSX project: NoteCard.tsx renders cards; format.zig formats counts. Run with
// `zig build run-tsx-notes`.
import { useState, useTheme, View, Row, Heading, Input, Button, Text } from "zant";
import type { u32 } from "zant";
import { NoteCard, type Note } from "./NoteCard";
import { plural } from "./format.zig";

export default function Notes() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [text, setText] = useState("");
  const [nextId, setNextId] = useState<u32>(1);

  function add() {
    if (text.length === 0) return;
    setNotes([...notes, { id: nextId, title: text, pinned: false }]);
    setNextId(nextId + 1);
    setText("");
  }

  function pin(id: u32) {
    setNotes(notes.map((n) => (n.id === id ? { ...n, pinned: !n.pinned } : n)));
  }

  const theme = useTheme();
  // pinned notes first
  const ordered = [...notes.filter((n) => n.pinned), ...notes.filter((n) => !n.pinned)];

  return (
    <View padding={24} gap={12}>
      <Heading level={3}>Notes</Heading>
      {notes.length > 0 && (
        <Text key="summary" color={theme.colorTextSecondary}>
          {plural(notes.length, "note")} · {notes.filter((n) => n.pinned).length} pinned
        </Text>
      )}
      <Row gap={8}>
        <View flex={1}>
          <Input key="new" value={text} placeholder="Write a note" onChange={setText} onSubmit={add} />
        </View>
        <Button key="add" variant="primary" onClick={add}>
          Add
        </Button>
      </Row>
      {notes.length === 0 ? (
        <Text key="empty" color={theme.colorTextSecondary}>No notes yet.</Text>
      ) : (
        <View key="notes" gap={4}>
          {ordered.map((n) => (
            <NoteCard key={n.id} note={n} onPin={pin} onDelete={(id) => setNotes(notes.filter((x) => x.id !== id))} />
          ))}
        </View>
      )}
    </View>
  );
}
