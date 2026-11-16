import { View, Button, Checkbox, Input, Select } from "zant";
import type { usize } from "zant";

function save(): void {}
function pick(id: number): void {}
function pair(id: number, label: string): void {}
const noop = () => {};
const rename = (value: string) => {};

function Defaults({
  onClose = () => {},
  onSave: submit = save,
  onSelect = noop,
  onPick = (id: number) => pick(id),
  onPair = (id, label) => { pair(id, label); },
  onToggle = (checked: boolean) => {},
  onRename = rename,
  onIndex = (index: usize) => {},
}: {
  onClose?: () => void;
  onSave?: () => void;
  onSelect?: () => void;
  onPick?: (id: number) => void;
  onPair?: (id: number, label: string) => void;
  onToggle?: (checked: boolean) => void;
  onRename?: (value: string) => void;
  onIndex?: (index: usize) => void;
}) {
  return <View>
    <Button onClick={onClose}>Close</Button>
    <Button onClick={() => { submit(); onSelect(); onPick(1); onPair(2, "x"); }}>Save</Button>
    <Checkbox checked={false} onChange={onToggle}>Toggle</Checkbox>
    <Input value="" onChange={onRename} />
    <Select options={["a", "b"]} selected={0} onChange={onIndex} />
  </View>;
}

export default function App() {
  return <View><Defaults /><Defaults onClose={() => save()} onPick={(id) => pick(id)} /></View>;
}
