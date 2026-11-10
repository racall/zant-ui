// A child component and its types, imported by App.tsx.
import { useTheme, Row, View, Text, IconButton } from "zant";
import type { u32 } from "zant";

export interface Note {
  id: u32;
  title: string;
  pinned: boolean;
}

export interface NoteCardProps {
  note: Note;
  onPin: (id: u32) => void;
  onDelete: (id: u32) => void;
}

export function NoteCard({ note, onPin, onDelete }: NoteCardProps) {
  const theme = useTheme();
  return (
    <Row gap={8} alignItems="center" padding={8} radius={8} hoverBackground={theme.colorFillTertiary}>
      <View flex={1}>
        <Text key={`title-${note.id}`} fontWeight={note.pinned ? "semibold" : "regular"}>
          {note.pinned ? `📌 ${note.title}` : note.title}
        </Text>
      </View>
      <IconButton key={`pin-${note.id}`} icon={note.pinned ? "arrow_down" : "arrow_up"} onClick={() => onPin(note.id)} />
      <IconButton key={`delete-${note.id}`} icon="trash" onClick={() => onDelete(note.id)} />
    </Row>
  );
}
