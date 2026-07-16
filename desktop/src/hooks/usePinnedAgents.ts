import { useCallback, useEffect, useRef, useState } from "react";
import { useToast } from "../components/Toast";
import { formatError } from "../lib/format";
import { getPinnedAgents, setPinnedAgents } from "../lib/api";

export interface PinnedAgentsState {
  agentIds: string[];
  saving: boolean;
  commit(agentIds: string[]): Promise<boolean>;
}

export function usePinnedAgents(): PinnedAgentsState {
  const [agentIds, setAgentIds] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);
  const savedRef = useRef<string[]>([]);
  const savingRef = useRef(false);
  const { show } = useToast();

  useEffect(() => {
    let active = true;
    getPinnedAgents()
      .then((loaded) => {
        if (!active) return;
        savedRef.current = loaded;
        setAgentIds(loaded);
      })
      .catch((error) => {
        if (active) show({ kind: "error", msg: `读取置顶 Agent 失败: ${formatError(error)}` });
      });
    return () => {
      active = false;
    };
  }, [show]);

  const commit = useCallback(async (nextIds: string[]) => {
    if (savingRef.current) return false;
    const previous = savedRef.current;
    savingRef.current = true;
    setSaving(true);
    setAgentIds(nextIds);
    try {
      const persisted = await setPinnedAgents(nextIds);
      savedRef.current = persisted;
      setAgentIds(persisted);
      return true;
    } catch (error) {
      setAgentIds(previous);
      show({ kind: "error", msg: `保存置顶 Agent 失败: ${formatError(error)}` });
      return false;
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  }, [show]);

  return { agentIds, saving, commit };
}
