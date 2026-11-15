import { createWindow, useCloseRequest, useStore, useTheme, View, Heading, Paragraph, Input, Button } from "zant";
import { profile } from "../stores/profile";

export function SettingsWindow() {
  const user = useStore(profile);
  const theme = useTheme();
  // 关闭时隐藏这个窗口，主窗口继续运行；再次打开复用同一个窗口。
  useCloseRequest(() => {
    settingsWindow.hide();
    return false;
  });
  return (
    <View background={theme.colorBgLayout} padding={24} gap={14}>
      <Heading level={2}>设置</Heading>
      <Paragraph>修改显示名称，主窗口的共享资料会同步更新。</Paragraph>
      <Paragraph>显示名称</Paragraph>
      <Input key="settings-profile-name" value={user.displayName} placeholder="显示名称"
        onChange={(name) => profile.update({ displayName: name })} />
      <Button key="close-settings-window" onClick={() => settingsWindow.hide()}>关闭设置窗口</Button>
    </View>
  );
}

// activate() 首次打开窗口，之后将已有窗口带到前面。
export const settingsWindow = createWindow(SettingsWindow, {
  title: "设置",
  style: "native",
  width: 440,
  height: 320,
  minWidth: 360,
  minHeight: 300,
  center: true,
});
