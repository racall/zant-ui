// Markdown streaming next to a PDF reader: two panes, the left one streams a
// reply, the right one opens a PDF. Run it with
// `cargo run -p zant-tsx-reader`.
import { useState, useTheme, View, Row, Text, Button, Input, Icon, Markdown, PdfViewer } from "zant";

export default function Reader() {
  const theme = useTheme();
  const [reply, setReply] = useState("# Native documents\n\nMarkdown and PDF, side by side.");
  const [chunks, setChunks] = useState(0);
  const [busy, setBusy] = useState(false);
  // the field is a draft; the viewer opens a file on Enter / Open
  const [draft, setDraft] = useState("tests/fixtures/widgets.pdf");
  const [path, setPath] = useState("tests/fixtures/widgets.pdf");

  function append() {
    setReply(reply + "\n\nA streamed chunk with **bold text**, `code`, and math $x^2 + y^2$.");
    setChunks(chunks + 1);
    setBusy(true);
  }

  function open() {
    setPath(draft.trim());
  }

  return (
    <View grow={1} padding={16} gap={16} background={theme.colorBgContainer}>
      <Row alignItems="center" gap={12}>
        <Text size={20} weight="semibold">
          Document reader
        </Text>
        <Text color={theme.colorTextSecondary}>Markdown · PDF</Text>
      </Row>
      <Row grow={1} gap={16} alignItems="stretch">
        <View flex={1} minWidth={280} gap={12}>
          <Row gap={8} alignItems="center">
            <Text weight="semibold">Markdown</Text>
            <Button key="append" variant="primary" onClick={append}>
              Append chunk
            </Button>
            <Button key="finish" disabled={!busy} onClick={() => setBusy(false)}>
              Finish
            </Button>
            <Text color={theme.colorTextSecondary}>
              {chunks} {chunks === 1 ? "chunk" : "chunks"}
            </Text>
          </Row>
          <View key="reply-scroll" grow={1} overflow="scroll">
            <Markdown key="reply" source={reply} streaming={busy} caret="block" fontSize={15} contentWidth={480} />
          </View>
        </View>
        <View flex={1} minWidth={280} gap={12}>
          <Text weight="semibold">PDF</Text>
          <Row gap={8}>
            <View flex={1}>
              <Input key="path" value={draft} placeholder="PDF path" onChange={setDraft} onSubmit={open} />
            </View>
            <Button key="open" onClick={open}>
              <Icon name="folder" /> Open
            </Button>
          </Row>
          <PdfViewer key="reader" src={path} toolbar annotationTools grow={1} />
        </View>
      </Row>
    </View>
  );
}
