import { Card, Text, Paragraph, Row, Button, useTheme } from "zant";

export interface CounterCardProps {
  count: number;
  onIncrement: () => void;
  onReset: () => void;
}

export function CounterCard({ count, onIncrement, onReset }: CounterCardProps) {
  const theme = useTheme();
  return (
    <Card title="跨组件状态" extra="props ↓ / events ↑">
      <Text size={28} weight="semibold" color={theme.colorPrimary}>点击次数：{count}</Text>
      <Paragraph>状态在父组件 App 中；子组件只发事件。</Paragraph>
      <Row gap={8}>
        <Button variant="primary" onClick={onIncrement}>+1</Button>
        <Button onClick={onReset}>重置</Button>
      </Row>
    </Card>
  );
}
