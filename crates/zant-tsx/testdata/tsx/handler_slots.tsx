// Components without hooks whose handlers are plain function references
// (`onClick={save}`): each handler is a method reached through the
// instance's state slot, so these components are built over one
// (`build(props, id)`). A component that only forwards its own callback prop
// stays slot-free.
import { createStore, useStore, View, Row, Text, Button, Switch } from "zant";

interface Counts {
  clicks: number;
  on: boolean;
}

const counts = createStore<Counts>({ clicks: 0, on: false });

function bump(): void {
  counts.update({ clicks: counts.get().clicks + 1 });
}

function toggle(value: boolean): void {
  counts.update({ on: value });
}

function BumpButton({ label }: { label: string }) {
  return <Button key="bump" onClick={bump}>{label}</Button>;
}

function Toggle() {
  return <Switch key="toggle" checked={counts.get().on} onChange={toggle} />;
}

function Forward({ onPress }: { onPress: () => void }) {
  return <Button key="forward" onClick={onPress}>Forward</Button>;
}

export default function App() {
  const s = useStore(counts);
  return (
    <View padding={16} gap={8}>
      <Text key="clicks">{`${s.clicks}`}</Text>
      <Row gap={8}>
        <BumpButton label="Bump" />
        <Toggle />
        <Forward onPress={bump} />
      </Row>
    </View>
  );
}
