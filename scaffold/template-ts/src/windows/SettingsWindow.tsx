import { createWindow, useCloseRequest, useStore, useTheme, useLocale, setLocale, localePreference, locales, localeName, t, View, Row, Heading, Paragraph, Input, Button } from "zant";
import { profile } from "../stores/profile";

export function SettingsWindow() {
  const user = useStore(profile);
  const theme = useTheme();
  const locale = useLocale();
  const following = localePreference() === "system";
  // Closing hides this window; the main window keeps running and reopening reuses it.
  useCloseRequest(() => {
    settingsWindow.hide();
    return false;
  });
  return (
    <View background={theme.colorBgLayout} padding={24} gap={14}>
      <Heading level={2}>{t("settings.title")}</Heading>
      <Paragraph>{t("settings.intro")}</Paragraph>
      <Input key="settings-profile-name" value={user.displayName} placeholder={t("profile.name")}
        onChange={(name) => profile.update({ displayName: name })} />
      <Paragraph>{t("settings.language")}</Paragraph>
      <Row gap={8}>
        <Button key="locale-system" variant={following ? "primary" : "default"}
          onClick={() => { setLocale("system"); }}>{t("settings.system")}</Button>
        {locales().map((tag) => (
          <Button key={`locale-${tag}`} variant={!following && tag === locale ? "primary" : "default"}
            onClick={() => { setLocale(tag); }}>{localeName(tag)}</Button>
        ))}
      </Row>
      <Button key="close-settings-window" onClick={() => settingsWindow.hide()}>{t("settings.close")}</Button>
    </View>
  );
}

// activate() opens the window the first time and brings it forward afterwards.
export const settingsWindow = createWindow(SettingsWindow, {
  title: "Settings",
  style: "native",
  width: 460,
  height: 380,
  minWidth: 380,
  minHeight: 340,
  center: true,
});
