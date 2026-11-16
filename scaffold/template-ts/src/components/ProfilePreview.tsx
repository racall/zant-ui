import { useStore, useTheme, Text, t } from "zant";
import { profile } from "../stores/profile";

export function ProfilePreview() {
  const user = useStore(profile);
  const theme = useTheme();
  return (
    <Text color={user.accent ? theme.colorPrimary : theme.colorText}>
      {t("profile.shared", { name: user.displayName })}
    </Text>
  );
}
