import { Column, Row, useTheme, t } from "zant";
import { info } from "native/system";
import { StatCard } from "./StatCard";
import { CounterCard } from "./CounterCard";

export interface DashboardProps {
  count: number;
  onIncrement: () => void;
  onReset: () => void;
}

// The parent passes its one state down instead of copying it; system info comes from a Rust native module.
export function Dashboard({ count, onIncrement, onReset }: DashboardProps) {
  const theme = useTheme();
  const sys = info();
  const platform = sys.os === "macos" ? "macOS" : sys.os === "windows" ? "Windows" : t("dashboard.other");
  return (
    <Column gap={12}>
      <Row gap={12}>
        <StatCard label={t("dashboard.platform")} value={platform} hint={t("dashboard.platformHint")} accent={theme.colorPrimary} />
        <StatCard label={t("dashboard.cores")} value={`${sys.cpus}`} hint={`Rust ${sys.rust}`} accent={theme.colorSuccess} />
      </Row>
      <CounterCard count={count} onIncrement={onIncrement} onReset={onReset} />
    </Column>
  );
}
