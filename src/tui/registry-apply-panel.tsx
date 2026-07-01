import { useState } from "react";
import { Box, Text, useInput, useStdout } from "ink";
import type { Scope } from "../types.js";
import { ShimmerBorder } from "./shimmer-border.js";

interface Props {
  mcpName: string;
  availableAgents: string[];
  onSave: (scope: Scope, targets: string[]) => void;
  onCancel: () => void;
}

type Step = "scope" | "targets";
const SCOPES: Scope[] = ["project", "global", "both"];

export function RegistryApplyPanel({ mcpName, availableAgents, onSave, onCancel }: Props) {
  const [step, setStep] = useState<Step>("scope");
  const [selectedScope, setSelectedScope] = useState<Scope>("project");
  const [scopeCursor, setScopeCursor] = useState(0);
  const [targets, setTargets] = useState<Set<string>>(new Set());
  const [targetCursor, setTargetCursor] = useState(0);
  const { stdout } = useStdout();
  const termWidth = stdout?.columns ?? 80;
  const panelWidth = termWidth;
  const contentRows = step === "scope" ? 10 : availableAgents.length + 7;

  useInput((input, key) => {
    if (key.escape) {
      if (step === "targets") {
        setStep("scope");
      } else {
        onCancel();
      }
      return;
    }

    if (step === "scope") {
      if (key.upArrow) {
        setScopeCursor((c) => Math.max(0, c - 1));
        return;
      }
      if (key.downArrow) {
        setScopeCursor((c) => Math.min(SCOPES.length - 1, c + 1));
        return;
      }
      if (key.return) {
        setSelectedScope(SCOPES[scopeCursor]);
        setStep("targets");
        return;
      }
    }

    if (step === "targets") {
      if (key.upArrow) {
        setTargetCursor((c) => Math.max(0, c - 1));
        return;
      }
      if (key.downArrow) {
        setTargetCursor((c) => Math.min(availableAgents.length - 1, c + 1));
        return;
      }
      if (input === " ") {
        const targetName = availableAgents[targetCursor];
        setTargets((prev) => {
          const next = new Set(prev);
          if (next.has(targetName)) next.delete(targetName);
          else next.add(targetName);
          return next;
        });
        return;
      }
      if (input === "a" && key.ctrl) {
        const allSelected = availableAgents.every((t) => targets.has(t));
        setTargets(allSelected ? new Set() : new Set(availableAgents));
        return;
      }
      if (key.return) {
        const selected = [...targets];
        if (selected.length > 0) {
          onSave(selectedScope, selected);
        }
        return;
      }
    }
  });

  return (
    <ShimmerBorder width={panelWidth} contentRows={contentRows} preset="cyan">
      <Box flexDirection="column" paddingX={1}>
        <Box marginBottom={1} gap={1}>
          <Text bold color="cyan">Apply:</Text>
          <Text bold>{mcpName}</Text>
        </Box>

        {step === "scope" && (
          <Box flexDirection="column">
            <Text dimColor>  Select scope:</Text>
            <Box flexDirection="column" marginTop={1}>
              {SCOPES.map((scope, i) => {
                const isCursor = scopeCursor === i;
                return (
                  <Box key={scope} gap={1}>
                    <Text color={isCursor ? "cyan" : undefined}>
                      {isCursor ? "›" : " "}
                    </Text>
                    <Text color={isCursor ? "cyan" : "white"}>{scope}</Text>
                  </Box>
                );
              })}
            </Box>
            <Box marginTop={1} gap={2}>
              <Text dimColor>↑↓</Text><Text>navigate</Text>
              <Text dimColor>Enter</Text><Text>select</Text>
              <Text dimColor>Esc</Text><Text>cancel</Text>
            </Box>
          </Box>
        )}

        {step === "targets" && (
          <Box flexDirection="column">
            <Text dimColor>  Scope: <Text color="yellow">{selectedScope}</Text> — Select targets:</Text>
            <Box flexDirection="column" marginTop={1}>
              {availableAgents.map((t, i) => {
                const isCursor = targetCursor === i;
                const isChecked = targets.has(t);
                return (
                  <Box key={t} gap={1}>
                    <Text color={isCursor ? "cyan" : undefined}>
                      {isCursor ? "›" : " "}
                    </Text>
                    <Text color={isChecked ? "green" : "gray"}>
                      {isChecked ? "◉" : "○"}
                    </Text>
                    <Text color={isCursor ? "cyan" : "white"}>{t}</Text>
                  </Box>
                );
              })}
            </Box>
            <Box marginTop={1} gap={2}>
              <Text dimColor>↑↓</Text><Text>navigate</Text>
              <Text dimColor>Space</Text><Text>toggle</Text>
              <Text dimColor>Ctrl+A</Text><Text>all</Text>
              <Text dimColor>Enter</Text><Text>apply</Text>
              <Text dimColor>Esc</Text><Text>back</Text>
            </Box>
          </Box>
        )}
      </Box>
    </ShimmerBorder>
  );
}
