import { useState, useStore, useTheme, View, Row, Heading, Paragraph, Input, Switch, Text, Button, t } from "zant";
import { Dashboard } from "./components/Dashboard";
import { ProfilePreview } from "./components/ProfilePreview";
import { profile } from "./stores/profile";
import { settingsWindow } from "./windows/SettingsWindow";

// One source of truth for state; children report events through callbacks.
// Text comes from locales/*.json through t(); TSX compiles to Rust at build time.
export default function App() {
  const title = "__APP_TITLE__";
  const user = useStore(profile);
  const [clicks, setClicks] = useState(0);
  const theme = useTheme();
  return (
    <View background={theme.colorBgLayout} gap={14}
      padding={{ top: 24 /* titlebar */, right: 24, bottom: 24, left: 24 }}>
      <Heading level={2}>{title}</Heading>
      <Paragraph>{t("app.intro")}</Paragraph>
      <Row gap={12} alignItems="center">
        <View grow={1}>
          <Input key="profile-name" value={user.displayName} placeholder={t("profile.name")}
            onChange={(name) => profile.update({ displayName: name })} />
        </View>
        <Switch key="profile-accent" label={t("profile.accent")} checked={user.accent}
          onChange={(on) => profile.update({ accent: on })} />
      </Row>
      <ProfilePreview />
      <Button key="open-settings-window" onClick={() => settingsWindow.activate()}>{t("app.openSettings")}</Button>
      <Text color={theme.colorTextSecondary}>{t("app.clicks", { count: clicks })}</Text>
      <Dashboard count={clicks} onIncrement={() => setClicks(clicks + 1)} onReset={() => setClicks(0)} />
    </View>
  );
}
