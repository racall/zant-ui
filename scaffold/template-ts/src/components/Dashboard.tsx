import { Column, Row, useTheme } from "zant";
import { info } from "native/system";
import { StatCard } from "./StatCard";
import { CounterCard } from "./CounterCard";

export interface DashboardProps {
  count: number;
  onIncrement: () => void;
  onReset: () => void;
}

// 父组件把同一份状态传给子组件，不复制状态；系统信息来自 Rust 原生模块。
export function Dashboard({ count, onIncrement, onReset }: DashboardProps) {
  const theme = useTheme();
  const sys = info();
  const platform = sys.os === "macos" ? "macOS" : sys.os === "windows" ? "Windows" : "其他";
  return (
    <Column gap={12}>
      <Row gap={12}>
        <StatCard label="平台" value={platform} hint="native/system.rs 返回" accent={theme.colorPrimary} />
        <StatCard label="CPU 核心" value={`${sys.cpus}`} hint={`Rust ${sys.rust}`} accent={theme.colorSuccess} />
      </Row>
      <CounterCard count={count} onIncrement={onIncrement} onReset={onReset} />
    </Column>
  );
}
