import { useState, useTheme, View, Row, Text, Button, Input, Markdown, PdfViewer } from "zant";

export default function Reader() {
  const theme = useTheme();
  const [reply, setReply] = useState("# Native documents\n\nMarkdown and PDF, side by side.");
  const [path, setPath] = useState("tests/fixtures/widgets.pdf");
  const [chunks, setChunks] = useState(0);
  const [busy, setBusy] = useState(false);
  function append() {
    setReply(reply + "\n\nA streamed chunk with **bold text**, `code`, and math $x^2 + y^2$.");
    setChunks(chunks + 1);
    setBusy(true);
  }
  return <View grow={1} padding={16} gap={16} background={theme.colorBgContainer}>
    <Row alignItems="center" gap={12}>
      <Text fontSize={20} fontWeight="semibold">Document reader</Text>
      <Text color={theme.colorTextSecondary}>Markdown · PDF</Text>
    </Row>
    <Row grow={1} gap={16} alignItems="stretch" wrap="wrap">
      <View flex={1} minWidth={280} gap={12}>
        <Row gap={8} alignItems="center" wrap="wrap">
          <Text fontWeight="semibold">Markdown</Text>
          <Button variant="primary" onClick={append}>Append chunk</Button>
          <Button disabled={!busy} onClick={() => setBusy(false)}>Finish</Button>
          <Text color={theme.colorTextSecondary}>{chunks} chunks</Text>
        </Row>
        <View grow={1} overflow="scroll">
          <Markdown key="reply" source={reply} streaming={busy} caret="block" fontSize={15} contentWidth={480} />
        </View>
      </View>
      <View flex={1} minWidth={280} gap={12}>
        <Text fontWeight="semibold">PDF</Text>
        <Input value={path} placeholder="PDF path" onChange={setPath} />
        <PdfViewer key="reader" src={path} toolbar annotationTools grow={1} />
      </View>
    </Row>
  </View>;
}
