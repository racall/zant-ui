// setTimeout / clearTimeout: a deadline that captures a number, a mount
// effect with a cleanup, and cancelling.
import { useState, useEffect, View, Text, Button } from "zant";

export default function App() {
  const [note, setNote] = useState("");
  const [left, setLeft] = useState(3);
  const [timer, setTimer] = useState(0);
  const start = () => {
    const row: number = left * 10;
    setTimer(setTimeout(() => {
      setNote(`expired ${row}`);
      setLeft(left - 1);
    }, 8000));
  };
  useEffect(() => {
    const t = setTimeout(() => setNote("mounted"), 10);
    return () => clearTimeout(t);
  }, []);
  // a timer inside an effect reads what the effect captured from the render
  const doubled = left * 2;
  useEffect(() => {
    const t = setTimeout(() => setNote(`doubled ${doubled}`), 20000);
    return () => clearTimeout(t);
  }, [doubled]);
  return (
    <View padding={16} gap={8}>
      <Text key="note">
        {note} {left}
      </Text>
      <Button key="start" onClick={start}>
        Start
      </Button>
      <Button key="stop" onClick={() => clearTimeout(timer)}>
        Stop
      </Button>
    </View>
  );
}
