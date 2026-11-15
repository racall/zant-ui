import { useState, useStore, useTheme, View, Row, Heading, Paragraph, Input, Switch, Text } from "zant";
import { Dashboard } from "./components/Dashboard";
import { ProfilePreview } from "./components/ProfilePreview";
import { profile } from "./stores/profile";

// 状态只有一个来源；子组件通过回调发事件。TSX 在编译期转成 Rust。
export default function App() {
  const title = "__APP_TITLE__";
  const user = useStore(profile);
  const [clicks, setClicks] = useState(0);
  const theme = useTheme();
  return (
    <View background={theme.colorBgLayout} gap={14}
      padding={{ top: 24 /* titlebar */, right: 24, bottom: 24, left: 24 }}>
      <Heading level={2}>{title}</Heading>
      <Paragraph>资料通过 store 共享；计数通过 props 和事件传递。</Paragraph>
      <Row gap={12} alignItems="center">
        <View grow={1}>
          <Input key="profile-name" value={user.displayName} placeholder="显示名称"
            onChange={(name) => profile.update({ displayName: name })} />
        </View>
        <Switch key="profile-accent" label="强调资料" checked={user.accent}
          onChange={(on) => profile.update({ accent: on })} />
      </Row>
      <ProfilePreview />
      <Text color={theme.colorTextSecondary}>父组件读取的点击次数：{clicks}</Text>
      <Dashboard count={clicks} onIncrement={() => setClicks(clicks + 1)} onReset={() => setClicks(0)} />
    </View>
  );
}
