import { useState, useEffect } from "react";
import { Box, Text } from "ink";

const LOGO_LINE_1 = " █▄ ▄█  █ █  ▀▄▀";
const LOGO_LINE_2 = " █ ▀ █  █▄█  ▄▀▄";
const SUBTITLE = "   MCP Multiplexer";

const SHIMMER_INTERVAL = 100;

// Static gradient: left=gold → mid=coral → right=magenta
const GRAD_STOPS = [
  [255, 200, 60],  // gold
  [255, 140, 100], // coral
  [230, 100, 200], // magenta
];

// White shimmer overlay (additive brightness boost)
const SHIMMER_BOOST = [
  0.05, 0.10, 0.18, 0.28, 0.40, 0.55, 0.70, 0.85, 0.95, 1.0,
  0.95, 0.85, 0.70, 0.55, 0.40, 0.28, 0.18, 0.10, 0.05,
];
const SHIMMER_LEN = SHIMMER_BOOST.length;

function lerp(a: number, b: number, t: number): number {
  return Math.round(a + (b - a) * t);
}

function rgbHex(r: number, g: number, b: number): string {
  return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
}

function gradientBase(charIndex: number, totalChars: number): [number, number, number] {
  const pos = totalChars <= 1 ? 0 : charIndex / (totalChars - 1);
  if (pos <= 0.5) {
    const t = pos / 0.5;
    return [
      lerp(GRAD_STOPS[0][0], GRAD_STOPS[1][0], t),
      lerp(GRAD_STOPS[0][1], GRAD_STOPS[1][1], t),
      lerp(GRAD_STOPS[0][2], GRAD_STOPS[1][2], t),
    ];
  }
  const t = (pos - 0.5) / 0.5;
  return [
    lerp(GRAD_STOPS[1][0], GRAD_STOPS[2][0], t),
    lerp(GRAD_STOPS[1][1], GRAD_STOPS[2][1], t),
    lerp(GRAD_STOPS[1][2], GRAD_STOPS[2][2], t),
  ];
}

export function Logo() {
  const [frame, setFrame] = useState(0);
  const maxCols = LOGO_LINE_2.length + SUBTITLE.length; // widest row
  const cycleLen = maxCols + SHIMMER_LEN + 10;

  useEffect(() => {
    const timer = setInterval(() => {
      setFrame((f) => (f + 1) % cycleLen);
    }, SHIMMER_INTERVAL);
    return () => clearInterval(timer);
  }, [cycleLen]);

  // Both gradient and shimmer use column position — so all rows align vertically
  const colorAt = (col: number): string => {
    const [r, g, b] = gradientBase(col, maxCols);
    const dist = col - frame;
    if (dist >= -SHIMMER_LEN && dist <= 0) {
      const boost = SHIMMER_BOOST[dist + SHIMMER_LEN] ?? 0;
      return rgbHex(
        Math.min(255, lerp(r, 255, boost)),
        Math.min(255, lerp(g, 255, boost)),
        Math.min(255, lerp(b, 255, boost)),
      );
    }
    return rgbHex(r, g, b);
  };

  const renderLine = (line: string, bold: boolean, colOffset: number): React.ReactNode[] => {
    return line.split("").map((ch, i) => (
      <Text key={i} color={colorAt(i + colOffset)} bold={bold}>{ch}</Text>
    ));
  };

  return (
    <Box flexDirection="column">
      <Box>{renderLine(LOGO_LINE_1, true, 0)}</Box>
      <Box>
        {renderLine(LOGO_LINE_2, true, 0)}
        {renderLine(SUBTITLE, false, LOGO_LINE_2.length)}
      </Box>
    </Box>
  );
}
