import { useState, useTheme, View, Heading, Paragraph, Input, Text } from "zant";
import { Dashboard } from "./components/Dashboard";

// 状态只有一个来源；子组件通过回调发事件。
export default function App() {
  const [name, setName] = useState("__APP_TITLE__");
  const [clicks, setClicks] = useState(0);
  const theme = useTheme();
  return (
    <View background={theme.colorBgLayout} gap={14}
      padding={{ top: 24 /* titlebar */, right: 24, bottom: 24, left: 24 }}>
      <Heading level={2}>{name}</Heading>
      <Paragraph>修改 App.tsx 和 components/ 试试。</Paragraph>
      <Input value={name} placeholder="项目名称" onChange={setName} />
      <Text color={theme.colorTextSecondary}>父组件读取的点击次数：{clicks}</Text>
      <Dashboard count={clicks} onIncrement={() => setClicks(clicks + 1)} onReset={() => setClicks(0)} />
    </View>
  );
}
