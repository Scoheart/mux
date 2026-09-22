import { useCallback, useEffect, useRef, useState } from "react";
import { useToast } from "../components/Toast";
import { formatError } from "../lib/format";
import { getPinnedAgents, setPinnedAgents } from "../lib/api";
import { usePreferenceRevision } from "../lib/preferenceObservation";

export interface PinnedAgentsState {
  agentIds: string[];
  ready: boolean;
  saving: boolean;
  commit(agentIds: string[]): Promise<boolean>;
}

export function usePinnedAgents(): PinnedAgentsState {
  const [agentIds, setAgentIds] = useState<string[]>([]);
  const [ready, setReady] = useState(false);
  const [saving, setSaving] = useState(false);
  const savedRef = useRef<string[]>([]);
  const readyRef = useRef(false);
  const savingRef = useRef(false);
  const { show } = useToast();
  const revision = usePreferenceRevision();
  const generation = useRef(0);

  useEffect(() => {
    let active = true;
    const request = ++generation.current;
    if (savingRef.current) return;
    getPinnedAgents()
      .then((loaded) => {
        if (!active || request !== generation.current || savingRef.current) return;
        savedRef.current = loaded;
        readyRef.current = true;
        setAgentIds(loaded);
        setReady(true);
      })
      .catch((error) => {
        if (active && request === generation.current) show({ kind: "error", msg: `读取置顶 Agent 失败: ${formatError(error)}` });
      });
    return () => {
      active = false;
    };
  }, [show, revision]);

  const commit = useCallback(async (nextIds: string[]) => {
    if (!readyRef.current || savingRef.current) return false;
    const previous = savedRef.current;
    savingRef.current = true;
    ++generation.current;
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

  return { agentIds, ready, saving, commit };
}
