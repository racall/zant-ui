import React, { useState, type ReactNode } from "react";
import * as R from "react";

function Child(): R.ReactNode {
  const runs = R.useRef(0);
  const label = R.useMemo(() => "child", []);
  R.useEffect(() => { runs.current += 1; }, []);
  return <Text>{label}</Text>;
}

export default function App(): ReactNode {
  const [n, setN] = React.useState(0);
  const [visible, setVisible] = useState(true);
  return (
    <React.Fragment>
      <Button onClick={() => setN(n + 1)}>{n}</Button>
      <Button onClick={() => setVisible(!visible)}>Toggle</Button>
      {visible && <Child />}
    </React.Fragment>
  );
}
