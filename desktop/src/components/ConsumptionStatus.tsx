import type { ConsumptionStatus as Status } from "../lib/types";
import { useTranslation } from "react-i18next";

export function ConsumptionStatus({
  status,
  reason,
}: {
  status: Status;
  reason?: string | null;
}) {
  const { t } = useTranslation();
  const labels: Record<Status, string> = {
    synced: t("observations.statuses.synced"),
    "external-added": t("observations.statuses.externalAdded"),
    "external-changed": t("observations.statuses.externalChanged"),
    "external-removed": t("observations.statuses.externalRemoved"),
    unparseable: t("observations.statuses.unparseable"),
    ambiguous: t("observations.statuses.ambiguous"),
    unsupported: t("observations.statuses.unsupported"),
  };
  return (
    <span
      className="mux-consumption-status"
      data-status={status}
      title={reason ?? labels[status]}
    >
      <span aria-hidden="true" />
      {labels[status]}
    </span>
  );
}
