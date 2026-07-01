import { useState, useEffect } from "react";
import { Box, Text, useStdout } from "ink";

const INTERVAL = 120;
const CYCLE_FRAMES = 40; // ~4.8s full cycle

// White breathing: dim gray ↔ bright white
const GRADIENT = [
  "#3a3a3a", "#404040", "#474747", "#4e4e4e", "#565656",
  "#5e5e5e", "#676767", "#707070", "#7a7a7a", "#848484",
  "#8f8f8f", "#9a9a9a", "#a5a5a5", "#b0b0b0", "#bbbbbb",
  "#c6c6c6", "#d1d1d1", "#dcdcdc", "#e7e7e7", "#f2f2f2",
  "#f2f2f2", "#e7e7e7", "#dcdcdc", "#d1d1d1", "#c6c6c6",
  "#bbbbbb", "#b0b0b0", "#a5a5a5", "#9a9a9a", "#8f8f8f",
  "#848484", "#7a7a7a", "#707070", "#676767", "#5e5e5e",
  "#565656", "#4e4e4e", "#474747", "#404040", "#3a3a3a",
];

const AUTHOR = " @Scoheart · v0.1.0 ";
const CORNER_R = "╯";
const H = "─";

interface Props {
  children: React.ReactNode;
}

export function BreathingBorder({ children }: Props) {
  const [frame, setFrame] = useState(0);
  const { stdout } = useStdout();
  const termWidth = stdout?.columns ?? 80;
  const termHeight = stdout?.rows ?? 24;

  useEffect(() => {
    const timer = setInterval(() => {
      setFrame((f) => (f + 1) % CYCLE_FRAMES);
    }, INTERVAL);
    return () => clearInterval(timer);
  }, []);

  const color = GRADIENT[frame] ?? GRADIENT[0];
  // Inner width = termWidth - 2 (border left + right)
  const innerW = termWidth - 2;
  const padLen = Math.max(0, innerW - AUTHOR.length);

  return (
    <Box flexDirection="column">
      <Box
        flexDirection="column"
        borderStyle="round"
        borderColor={color}
        width={termWidth}
        height={termHeight - 1}
        borderBottom={false}
      >
        {children}
      </Box>
      <Box>
        <Text color={color}>╰</Text>
        <Text color={color}>{H.repeat(padLen)}</Text>
        <Text color="#555">{AUTHOR}</Text>
        <Text color={color}>{CORNER_R}</Text>
      </Box>
    </Box>
  );
}
