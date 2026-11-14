// useWake: a native module's background result read on the UI thread.
import { useState, useWake, useEffect, View, Text } from "zant";
import { start, take } from "./jobs.rs";

export function Work() {
  return <Text key="work">working</Text>;
}

export default function WakeApp() {
  const [value, setValue] = useState<u32>(0);
  useEffect(() => {
    start(21);
  }, []);
  useWake(() => {
    const got = take();
    if (got > 0) setValue(got);
  });
  return (
    <View padding={8}>
      <Text key="value">{value}</Text>
    </View>
  );
}
