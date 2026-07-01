import { useState, useEffect } from "react";
import { Box, Text, useInput } from "ink";
import Spinner from "ink-spinner";

export interface ScannedItem {
  name: string;
  type: "stdio" | "http";
  target: string;
  scope: string;
}

interface Props {
  items: ScannedItem[];
  onComplete: (action: "import" | "skip") => void;
}

const ITEM_DELAY = 120;

export function ScanView({ items, onComplete }: Props) {
  const [visibleCount, setVisibleCount] = useState(0);
  const [scanPhase, setScanPhase] = useState<"scanning" | "done">("scanning");
  const [selected, setSelected] = useState<"import" | "skip">("import");

  const projectItems = items.filter((i) => i.scope === "project");
  const globalItems = items.filter((i) => i.scope === "global");

  useEffect(() => {
    if (items.length === 0) {
      setScanPhase("done");
      return;
    }

    let current = 0;
    const timer = setInterval(() => {
      current++;
      setVisibleCount(current);
      if (current >= items.length) {
        clearInterval(timer);
        setScanPhase("done");
      }
    }, ITEM_DELAY);

    return () => clearInterval(timer);
  }, [items.length]);

  useInput((_input, key) => {
    if (scanPhase !== "done") return;
    if (key.leftArrow || key.rightArrow) {
      setSelected((prev) => (prev === "import" ? "skip" : "import"));
    }
    if (key.return) {
      onComplete(selected);
    }
  });

  const nameWidth = Math.max(...items.map((i) => i.name.length), 10) + 2;
  const typeWidth = 7;

  const visibleProject = projectItems.slice(0, Math.max(0, visibleCount));
  const visibleGlobal = globalItems.slice(0, Math.max(0, visibleCount - projectItems.length));

  return (
    <Box flexDirection="column">
      {/* Logo */}
      <Box marginBottom={0}>
        <Text color="cyan" bold>{"  __  __            "}</Text>
      </Box>
      <Box>
        <Text color="cyan" bold>{" |  \\/  |_   ___  __"}</Text>
      </Box>
      <Box>
        <Text color="cyan" bold>{" | |\\/| | | | \\ \\/ /"}</Text>
      </Box>
      <Box marginBottom={1}>
        <Text color="cyan" bold>{" |_|  |_|_,_|_|>  < "}</Text>
      </Box>

      {/* Scanning status */}
      <Box marginBottom={1}>
        {scanPhase === "scanning" ? (
          <Box gap={1}>
            <Text color="cyan"><Spinner type="dots" /></Text>
            <Text>Scanning your MCP configurations...</Text>
          </Box>
        ) : (
          <Box gap={1}>
            <Text color="green">✓</Text>
            <Text bold>Found {items.length} MCP servers</Text>
          </Box>
        )}
      </Box>

      {/* Project group */}
      {(visibleProject.length > 0 || scanPhase === "done") && projectItems.length > 0 && (
        <Box flexDirection="column">
          <Text color="yellow">  ┌ Project ({projectItems.length})</Text>
          <Text color="yellow">  │</Text>
          {visibleProject.map((item, i) => (
            <Box key={`p-${item.name}-${item.target}-${i}`}>
              <Text color="yellow">  │  </Text>
              <Box width={nameWidth}>
                <Text bold color="white">{item.name}</Text>
              </Box>
              <Box width={typeWidth}>
                <Text color="gray">{item.type}</Text>
              </Box>
              <Text dimColor>← </Text>
              <Text color="blue">{item.target}</Text>
            </Box>
          ))}
          <Text color="yellow">  │</Text>
          <Text color="yellow">  └{"─".repeat(nameWidth + typeWidth + 10)}</Text>
        </Box>
      )}

      {/* Global group */}
      {(visibleGlobal.length > 0 || (scanPhase === "done" && globalItems.length > 0)) && (
        <Box flexDirection="column" marginTop={1}>
          <Text color="magenta">  ┌ Global ({globalItems.length})</Text>
          <Text color="magenta">  │</Text>
          {visibleGlobal.map((item, i) => (
            <Box key={`g-${item.name}-${item.target}-${i}`}>
              <Text color="magenta">  │  </Text>
              <Box width={nameWidth}>
                <Text bold color="white">{item.name}</Text>
              </Box>
              <Box width={typeWidth}>
                <Text color="gray">{item.type}</Text>
              </Box>
              <Text dimColor>← </Text>
              <Text color="blue">{item.target}</Text>
            </Box>
          ))}
          <Text color="magenta">  │</Text>
          <Text color="magenta">  └{"─".repeat(nameWidth + typeWidth + 10)}</Text>
        </Box>
      )}

      {/* Progress indicator while scanning */}
      {scanPhase === "scanning" && visibleCount < items.length && (
        <Box marginTop={1}>
          <Text dimColor>  {visibleCount}/{items.length} discovered...</Text>
        </Box>
      )}

      {/* Action buttons */}
      {scanPhase === "done" && (
        <Box marginTop={1} gap={2}>
          <Text dimColor>  ←→ select  Enter confirm    </Text>
          {selected === "import" ? (
            <Text bold inverse color="green"> Import </Text>
          ) : (
            <Text dimColor>Import</Text>
          )}
          {selected === "skip" ? (
            <Text bold inverse color="yellow"> Skip </Text>
          ) : (
            <Text dimColor>Skip</Text>
          )}
        </Box>
      )}
    </Box>
  );
}
