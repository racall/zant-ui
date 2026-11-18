//! TSX sources of the regressions, shared by the lowering checks and the `cargo check` run.

/// 1. Child callbacks: a child handler calls the parent's callback prop with
///    the runtime context, synchronously, including through a helper function.
pub(super) const CHILD_CALLBACKS: &str = r#"
import { useState, View, Row, Text, Button, Checkbox } from "zant";
import type { u32 } from "zant";

interface StepProps {
  label: string;
  onStep: () => void;
  onPick: (id: u32) => void;
  onRename: (value: string) => void;
  onToggle: (checked: boolean) => void;
}

function Stepper({ label, onStep, onPick, onRename, onToggle }: StepProps) {
  const [n, setN] = useState<u32>(0);
  function add(delta: u32) {
    setN(n + delta);
    onStep();
  }
  return (
    <Row gap={8}>
      <Text key="label">{label}</Text>
      <Button key="plus" onClick={() => add(1)}>+</Button>
      <Button key="pick" onClick={() => onPick(7)}>Pick</Button>
      <Button key="rename" onClick={() => onRename("x")}>Rename</Button>
      <Checkbox key="toggle" checked={false} onChange={(checked) => onToggle(checked)}>T</Checkbox>
    </Row>
  );
}

export default function App() {
  const [count, setCount] = useState<u32>(0);
  const [picked, setPicked] = useState<u32>(0);
  const [name, setName] = useState("");
  return (
    <View>
      <Stepper label="Steps" onStep={() => setCount(count + 1)} onPick={(id) => setPicked(id)}
        onRename={(value) => setName(value)} onToggle={(checked) => setPicked(checked ? 1 : 0)} />
      <Text>{count} {picked} {name}</Text>
    </View>
  );
}
"#;

/// 1b. The same through a non-destructured `props` parameter (`props.onSave(..)`).
pub(super) const PROPS_OBJECT_CALLBACKS: &str = r#"
import { useState, View, Row, Text, Button } from "zant";
import type { u32 } from "zant";

interface ChildProps {
  title: string;
  onSave: () => void;
  onPick: (id: u32) => void;
}

function Child(props: ChildProps) {
  return (
    <Row>
      <Text>{props.title}</Text>
      <Button key="save" onClick={() => props.onSave()}>Save</Button>
      <Button key="pick" onClick={() => props.onPick(3)}>Pick</Button>
    </Row>
  );
}

export default function App() {
  const [n, setN] = useState<u32>(0);
  return (
    <View>
      <Child title="x" onSave={() => setN(n + 1)} onPick={(id) => setN(id)} />
      <Text>{n}</Text>
    </View>
  );
}
"#;

/// 2. `Option<Node>` prop with a bare element.
pub(super) const OPTIONAL_NODE: &str = r#"
import { useState, Modal, Row, Button, View, Paragraph } from "zant";

export default function App() {
  const [open, setOpen] = useState(false);
  return (
    <View>
      {open && (
        <Modal key="dialog" title="Save changes" onClose={() => setOpen(false)}
          footer={<Row gap={8} justifyContent="flex-end"><Button key="cancel" onClick={() => setOpen(false)}>Cancel</Button></Row>}>
          <Paragraph>hi</Paragraph>
        </Modal>
      )}
    </View>
  );
}
"#;

/// 3. Rich text with a literal after an expression child.
pub(super) const RICH_TEXT_TAIL: &str = r#"
import { useState, Text, Link } from "zant";
import type { u32 } from "zant";

export default function App() {
  const [docs, setDocs] = useState<u32>(0);
  return (
    <Text key="rich">
      inline <Link onClick={() => setDocs(docs + 1)}>link</Link>
      {docs > 0 && ` (opened ${docs}×)`}.
    </Text>
  );
}
"#;

/// 4. `<List>` inside a component with props and local state, whose row
///    handler updates that state and calls a callback prop.
pub(super) const LIST_IN_CHILD: &str = r#"
import { useState, List, View, Row, Text, Button } from "zant";
import type { u32 } from "zant";

interface RowData { id: u32; title: string; }

export function ListView({ rows, onPick }: { rows: RowData[]; onPick: (id: u32) => void }) {
  const [picked, setPicked] = useState<u32>(0);
  return (
    <View>
      <Text>Picked {picked}</Text>
      <List data={rows} itemHeight={40} keyExtractor={(row) => row.id}
        renderItem={(row) => (
          <Row>
            <Text>{row.title}</Text>
            <Button onClick={() => { setPicked(row.id); onPick(row.id); }}>Pick</Button>
          </Row>
        )} />
    </View>
  );
}

export default function App() {
  const [picked, setPicked] = useState<u32>(0);
  return (
    <View>
      <ListView rows={[{ id: 1, title: "a" }]} onPick={(id) => setPicked(id)} />
      <Text>{picked}</Text>
    </View>
  );
}
"#;

/// 5. Optional `children` props: a list with a default and a single element,
///    given as JSX children or left out.
pub(super) const OPTIONAL_CHILDREN: &str = r#"
import { Column, Row, Text } from "zant";

interface CardProps {
  title: string;
  children?: JSX.Element[];
}

function Card({ title, children = [] }: CardProps) {
  return <Column><Text>{title}</Text>{children}</Column>;
}

interface FrameProps {
  children?: JSX.Element;
}

function Frame({ children }: FrameProps) {
  return <Row>{children}</Row>;
}

export default function App() {
  return (
    <Column>
      <Card title="two"><Text>a</Text><Text>b</Text></Card>
      <Card title="none" />
      <Frame><Text>c</Text></Frame>
      <Frame />
    </Column>
  );
}
"#;

pub(super) const LOCAL_DESTRUCTURE_RENAME: &str = r#"
import { View, Text, Button, useState, useTheme } from "zant";
interface Item { title: string; id: number; }
function makeItem(): Item { return { title: "Hello", id: 7 }; }
function localLabel(item: Item): string {
  const { title: label, id } = item;
  return `${id}: ${label} ${item.title}`;
}
function useTitle() {
  const [title, setTitle] = useState("Hook");
  return { title, rename: () => setTitle("Renamed") };
}
export default function App() {
  const [text, setText] = useState("");
  const item: Item = makeItem();
  const { title: label, id } = item;
  const inlineItem: { title: string } = { title: "Inline" };
  const { title: inlineLabel } = inlineItem;
  const { title: again } = makeItem();
  let { title: mutable }: Item = item;
  mutable = "Changed";
  const theme = useTheme();
  const { colorText: ink } = theme;
  const { title: hookTitle, rename: renameHook } = useTitle();
  function rename() {
    const { title: next } = makeItem();
    setText(next);
  }
  return <View><Text color={ink}>{label} {inlineLabel} {id} {again} {mutable} {text} {localLabel(item)}</Text>
    <Button onClick={rename}>Rename</Button>
    <Button onClick={renameHook}>{hookTitle}</Button></View>;
}
"#;
