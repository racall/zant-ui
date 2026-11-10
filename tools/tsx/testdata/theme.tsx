import { useState, useTheme, View, Row, Text, Button } from "zant";

export default function Themed() {
  const theme = useTheme();
  const [active, setActive] = useState(false);
  const [last, setLast] = useState("");

  return (
    <View padding={theme.paddingLg} gap={theme.paddingSm} background={theme.colorBgContainer} radius={theme.borderRadiusLg}>
      <Text fontSize={theme.fontSizeLg} color={active ? theme.colorPrimary : theme.colorText}>
        {active ? "Active" : "Idle"}
      </Text>
      <Text color={theme.colorTextSecondary}>Nothing selected yet.</Text>
      <Row gap={theme.paddingSm} borderColor={theme.colorBorder} borderWidth={1} radius={theme.borderRadius}>
        <Button key="toggle" onClick={() => setActive(!active)}>
          Toggle
        </Button>
        <Button key="log" onClick={() => setLast(`${theme.motionDurationMid} ms`)}>
          Log
        </Button>
      </Row>
      {last.length > 0 && <Text color={theme.colorSuccessText}>{last}</Text>}
    </View>
  );
}
