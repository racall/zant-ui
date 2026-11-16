import { Card, Text, Paragraph, Row, Button, useTheme, t } from "zant";

export interface CounterCardProps {
  count: number;
  onIncrement: () => void;
  onReset: () => void;
}

export function CounterCard({ count, onIncrement, onReset }: CounterCardProps) {
  const theme = useTheme();
  return (
    <Card title={t("counter.title")} extra="props ↓ / events ↑">
      <Text size={28} weight="semibold" color={theme.colorPrimary}>{t("counter.count", { count })}</Text>
      <Paragraph>{t("counter.note")}</Paragraph>
      <Row gap={8}>
        <Button variant="primary" onClick={onIncrement}>+1</Button>
        <Button onClick={onReset}>{t("counter.reset")}</Button>
      </Row>
    </Card>
  );
}
