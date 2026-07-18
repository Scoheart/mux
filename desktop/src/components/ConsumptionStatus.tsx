import type { ConsumptionStatus as Status } from "../lib/types";

const LABELS: Record<Status, string> = {
  synced: "已同步",
  pending: "待同步",
  drifted: "有漂移",
  conflicted: "有冲突",
  unsupported: "不兼容",
  external: "外部配置",
};

export function ConsumptionStatus({
  status,
  reason,
}: {
  status: Status;
  reason?: string | null;
}) {
  return (
    <span
      className="mux-consumption-status"
      data-status={status}
      title={reason ?? LABELS[status]}
    >
      <span aria-hidden="true" />
      {LABELS[status]}
    </span>
  );
}
