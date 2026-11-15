import { useStore, useTheme, Text } from "zant";
import { profile } from "../stores/profile";

export function ProfilePreview() {
  const user = useStore(profile);
  const theme = useTheme();
  return (
    <Text color={user.accent ? theme.colorPrimary : theme.colorText}>
      共享资料：{user.displayName}
    </Text>
  );
}
