import { Card, Text, useTheme } from "zant";
import type { u32 } from "zant";

export interface StatCardProps {
  label: string;
  value: string;
  hint: string;
  accent: u32;
}

export function StatCard({ label, value, hint, accent }: StatCardProps) {
  const theme = useTheme();
  return (
    <Card width={248} height={150} gap={6} padding={16}>
      <Text size={12} color={theme.colorTextSecondary}>{label}</Text>
      <Text size={28} weight="semibold" color={accent}>{value}</Text>
      <Text size={12} color={theme.colorTextSecondary}>{hint}</Text>
    </Card>
  );
}
