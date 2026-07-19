import type { ConsumptionView, ModelProfileView } from "./types";

export interface AgentModelDisplay {
  label: string;
  detail: string;
  synced: boolean;
}

export function describeAgentModel(
  profile: ModelProfileView | null,
  consumption: ConsumptionView | null,
  hasExternalConfig: boolean,
): AgentModelDisplay {
  if (profile && consumption?.status === "synced") {
    return { label: profile.name, detail: profile.model, synced: true };
  }
  if (consumption) {
    const expected = profile ? `期望：${profile.name} · ${profile.model}` : "中央模型资产已缺失";
    if (consumption.status === "drifted") {
      return consumption.reason === "model_target_missing"
        ? { label: "配置缺失", detail: expected, synced: false }
        : { label: "配置已变更", detail: expected, synced: false };
    }
    if (consumption.status === "conflicted") {
      return { label: "配置冲突", detail: expected, synced: false };
    }
    if (consumption.status === "unsupported") {
      return { label: "当前配置不兼容", detail: expected, synced: false };
    }
    if (consumption.status === "pending") {
      return { label: "等待同步", detail: expected, synced: false };
    }
  }
  if (hasExternalConfig) {
    return { label: "外部配置", detail: "未纳入中央模型库", synced: false };
  }
  return { label: "未配置", detail: "尚未选择模型", synced: false };
}
