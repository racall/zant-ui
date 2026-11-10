import { Column, Row, useTheme } from "zant";
import { StatCard } from "./StatCard";
import { CounterCard } from "./CounterCard";

export interface DashboardProps {
  count: number;
  onIncrement: () => void;
  onReset: () => void;
}

// 父组件把同一份状态传给子组件，不复制状态。
export function Dashboard({ count, onIncrement, onReset }: DashboardProps) {
  const theme = useTheme();
  return (
    <Column gap={12}>
      <Row gap={12}>
        <StatCard label="构建次数" value="128" hint="父组件传给子组件的 props" accent={theme.colorPrimary} />
        <StatCard label="组件" value="12" hint="由 Zant 组件组合而成" accent={theme.colorSuccess} />
      </Row>
      <CounterCard count={count} onIncrement={onIncrement} onReset={onReset} />
    </Column>
  );
}
