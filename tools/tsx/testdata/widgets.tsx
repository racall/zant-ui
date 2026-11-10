import { useState, View, Markdown, PdfViewer } from "zant";

function Reply({ source }: { source: string }) {
  return <Markdown key="reply" streaming caret="block" fontSize={15}>{source}</Markdown>;
}

function Document({ src, onChange }: { src: string; onChange: () => void }) {
  return <PdfViewer key="document" src={src} toolbar annotationTools onChange={onChange} height={240} />;
}

export default function App() {
  const [source, setSource] = useState("# Title\n\nFirst paragraph.\n\nSecond **bo");
  const [path, setPath] = useState("tests/fixtures/widgets.pdf");
  const [visible, setVisible] = useState(true);
  const [changes, setChanges] = useState(0);
  return <View gap={8}>
    <Markdown key={7} source="Independent **cache**" />
    <PdfViewer src="" height={80} onChange={() => setChanges(changes + 1)} />
    {visible && <Reply key="answer" source={source} />}
    {visible && <Document key="file" src={path} onChange={() => setChanges(changes + 1)} />}
  </View>;
}
