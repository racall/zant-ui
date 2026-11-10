import { useState, View, Text } from "zant";

export default function Conditional(): ReactNode {
  const [visible, setVisible] = useState(true);
  const [count, setCount] = useState<i32>(-1);
  const header = <Text>Always present</Text>;
  const optional = visible ? <Text>Optional local</Text> : null;
  const number = visible ? 1 : 2;
  if (!visible) return <Text>Hidden</Text>;
  if (count < -5) {
    const ui = <Text>Early branch</Text>;
    return ui;
  } else {
    const unused = count;
  }
  return (
    <View>
      {header}
      {visible && <Text>Visible</Text>}
      {visible ? <Text>A</Text> : <Text>B</Text>}
      {visible ? <Text>C</Text> : null}
      {optional}
      {visible && <><Text>D</Text><Text>E</Text></>}
      {visible ? <><Text>F</Text>{count < 0 && <><Text>G</Text></>}</> : <Text>H</Text>}
      <Text>{number} {count / 2} {count % 2} {visible && count < 0 || !visible}</Text>
      {/* comment */}{}
    </View>
  );
}
