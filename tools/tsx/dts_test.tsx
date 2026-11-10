// Editor contract: run tsc --noEmit -p tools/tsx (no runtime or GUI).
import {
  Button, View, Text, Link, Input, Checkbox, Tooltip, Icon, Image, List, Markdown, PdfViewer, Tabs,
  useState, useEffect, useMemo, useRef, useTheme, createStore, useStore, createWindow,
  useCloseRequest, useQuitRequest, useKeyDown, useWindowFocus, useClipboardChange, useReopen,
  useWake, useTray, useHotkey,
} from "zant";
import type { u32, f64, Image as NativeImage } from "zant";
import { nativeFunction } from "./editor-test.zig";
import icon from "./editor-test.png";

const store = createStore<{ n: u32 }>({ n: 0 });
const panel = createWindow(Panel, { style: "panel", width: 300, background: "#fff" });
function Panel() { return <Text>Panel</Text>; }

export function EditorContract() {
  const [n, setN] = useState<f64>(0);
  const [checked, setChecked] = useState(false);
  const [value, setValue] = useState("");
  const ref = useRef<u32>(0);
  const doubled = useMemo(() => n * 2, [n]);
  const theme = useTheme();
  const count = useStore(store, s => s.n);
  const entire = useStore(store);
  useEffect(() => { ref.current = n; return () => { ref.current = 0; }; }, [n]);
  useCloseRequest(() => true);
  useQuitRequest(() => false);
  useKeyDown(e => e.mod && !e.shift && !e.alt && !e.ctrl && !e.meta && !e.repeat && e.key === "a");
  useWindowFocus(focused => { if (!focused) panel.hide(); });
  useClipboardChange(() => setN(v => v + 1));
  useReopen(() => panel.activate());
  useWake(() => store.update({ n: store.get().n + 1 }));
  useHotkey("mod+v", () => panel.toggle());
  useTray({ icon, iconLight: icon, template: true, tooltip: "Test", items: [
    { title: "Show", onSelect: () => panel.show() }, { separator: true },
    { title: "Enabled", checked, disabled: false, onSelect: () => setChecked(!checked) },
  ] });
  panel.place("trayCorner", 12);
  panel.focus("input");
  panel.visible();
  store.set({ n: count + entire.n });
  const image: NativeImage = nativeFunction();
  return <View key={1} width="50%" padding={{ top: 1, horizontal: 12 }} style={{ gap: 8 }}>
    <Text fontWeight={600} color={theme.colorPrimary}>
      Count: {n} <Text italic underline strike background="#fff">{doubled}</Text>
      <Link onClick={() => setN(0)}>reset</Link>
    </Text>
    <Input key="input" value={value} onChange={setValue} />
    <Checkbox checked={checked} onChange={setChecked}>Enabled</Checkbox>
    <Button variant="primary" onClick={() => setN(n + 1)}><Icon name="check" /> Add</Button>
    <Tooltip text="Help"><Button>Hover</Button></Tooltip>
    <Image image={image} fit="contain" />
    <Tabs items={[{ label: "A", content: <Text>A</Text> }]} onChange={index => setN(index)} />
    <List data={[{ id: 1, label: "A" }]} keyExtractor={item => item.id}
      renderItem={(item, index) => <Text key={item.id}>{item.label}: {index}</Text>} />
    <Markdown>{"# Heading"}</Markdown>
    <PdfViewer src="document.pdf" onChange={() => setN(1)} />
    {checked && <Text>Checked</Text>}
    {[1, 2].map(item => <Text key={item}>{item}</Text>)}
  </View>;
}

// @ts-expect-error Lowercase tags are never accepted.
const lowercase = <button />;
// @ts-expect-error Unknown component props.
const unknownProp = <Button madeUp />;
// @ts-expect-error Enum spellings come from the tag table.
const badEnum = <Button variant="invented" />;
// @ts-expect-error Flag spellings also come from the tag table.
const badFlag = <View overflow="auto" />;
// @ts-expect-error A bool callback receives boolean, not string.
const badCallback = <Checkbox onChange={(value: string) => {}} />;
// @ts-expect-error Leaf components do not accept children.
const badLeaf = <Input><Text>child</Text></Input>;
// @ts-expect-error Container text must be inside Text.
const badContainer = <View>plain text</View>;
// @ts-expect-error Required metadata fields stay required.
const missingIcon = <Icon />;
// @ts-expect-error Required fields of object-array items stay required.
const badItem = <Tabs items={[{ label: "Missing content" }]} />;
// @ts-expect-error PdfViewer requires a source path.
const missingPdf = <PdfViewer />;
// @ts-expect-error Tooltip requires its child or children.
const missingChild = <Tooltip text="Hint" />;
// @ts-expect-error Markdown requires source or children.
const missingMarkdown = <Markdown />;
