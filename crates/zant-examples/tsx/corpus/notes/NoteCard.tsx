// A child component and its types, imported by App.tsx.
import { useState, useTheme, Row, View, Text, IconButton } from "zant";
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

// `open` is this card's own state (phase 3): every card remembers it, and it
// goes away with the card
export function NoteCard({ note, onPin, onDelete }: NoteCardProps) {
  const theme = useTheme();
  const [open, setOpen] = useState(false);
  return (
    <Row gap={8} alignItems="center" padding={8} radius={8} hoverBackground={theme.colorFillTertiary}>
      <IconButton
        key={`open-${note.id}`}
        icon={open ? "chevron_down" : "chevron_right"}
        onClick={() => setOpen(!open)}
      />
      <View flex={1} gap={2}>
        <Text key={`title-${note.id}`} fontWeight={note.pinned ? "semibold" : "regular"}>
          {note.pinned ? `📌 ${note.title}` : note.title}
        </Text>
        {open && (
          <Text key={`detail-${note.id}`} color={theme.colorTextSecondary} fontSize={12}>
            Note #{note.id} · {note.title.length} characters{note.pinned ? " · pinned" : ""}
          </Text>
        )}
      </View>
      <IconButton
        key={`pin-${note.id}`}
        icon={note.pinned ? "arrow_down" : "arrow_up"}
        onClick={() => onPin(note.id)}
      />
      <IconButton key={`delete-${note.id}`} icon="trash" onClick={() => onDelete(note.id)} />
    </Row>
  );
}
