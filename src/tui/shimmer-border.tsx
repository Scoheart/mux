import { useState, useEffect } from "react";
import { Box, Text } from "ink";

const INTERVAL = 80;

const TL = "╭";
const TR = "╮";
const BL = "╰";
const BR = "╯";
const H = "─";
const V = "│";

export type ShimmerPreset = "cyan" | "yellow";

// Longer shimmer tails (25+ frames) for a more dramatic sweep
const PRESETS: Record<ShimmerPreset, { baseDim: number[]; baseBright: number[]; shimmer: string[] }> = {
  cyan: {
    baseDim: [30, 35, 45],
    baseBright: [55, 75, 95],
    shimmer: [
      "#334", "#345", "#356", "#368", "#47a", "#58c", "#69d", "#7ae",
      "#8bf", "#9cf", "#adf", "#bef", "#cff", "#bef", "#adf",
      "#9cf", "#8bf", "#7ae", "#69d", "#58c", "#47a", "#368",
      "#356", "#345", "#334",
    ],
  },
  yellow: {
    baseDim: [40, 38, 28],
    baseBright: [90, 85, 60],
    shimmer: [
      "#443", "#453", "#564", "#675", "#786", "#897", "#9a8",
      "#ab9", "#bca", "#cdb", "#dec", "#efd", "#ffe", "#efd",
      "#dec", "#cdb", "#bca", "#ab9", "#9a8", "#897", "#786",
      "#675", "#564", "#453", "#443",
    ],
  },
};

const BREATH_CYCLE_FRAMES = 36; // ~2.9s at 80ms interval

function lerpChannel(lo: number, hi: number, t: number): number {
  return Math.round(lo + (hi - lo) * t);
}

function rgbToHex(r: number, g: number, b: number): string {
  return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
}

interface Props {
  width: number;
  contentRows: number;
  preset?: ShimmerPreset;
  children: React.ReactNode;
}

export function ShimmerBorder({ width, contentRows, preset, children }: Props) {
  const innerW = Math.max(0, width - 2);
  const config = preset ? PRESETS[preset] : PRESETS.cyan;
  const shimmer = config.shimmer;
  const slen = shimmer.length;

  const perimeter = 2 * (innerW + 2) + 2 * contentRows;
  const cycleLen = perimeter + slen + 6;

  const [frame, setFrame] = useState(0);
  const [breathFrame, setBreathFrame] = useState(0);

  useEffect(() => {
    const timer = setInterval(() => {
      setFrame((f) => (f + 1) % cycleLen);
      setBreathFrame((b) => (b + 1) % BREATH_CYCLE_FRAMES);
    }, INTERVAL);
    return () => clearInterval(timer);
  }, [cycleLen]);

  // Breathing: sine wave 0→1→0 over BREATH_CYCLE_FRAMES
  const breathT = (Math.sin((breathFrame / BREATH_CYCLE_FRAMES) * Math.PI * 2 - Math.PI / 2) + 1) / 2;
  const baseColor = rgbToHex(
    lerpChannel(config.baseDim[0], config.baseBright[0], breathT),
    lerpChannel(config.baseDim[1], config.baseBright[1], breathT),
    lerpChannel(config.baseDim[2], config.baseBright[2], breathT),
  );

  const colorAt = (pos: number): string => {
    const dist = pos - frame;
    if (dist >= -slen && dist <= 0) {
      return shimmer[dist + slen] ?? baseColor;
    }
    return baseColor;
  };

  let pos = 0;

  const topRow: React.ReactNode[] = [];
  topRow.push(<Text key="tl" color={colorAt(pos)}>{TL}</Text>);
  pos++;
  for (let i = 0; i < innerW; i++) {
    topRow.push(<Text key={`t${i}`} color={colorAt(pos)}>{H}</Text>);
    pos++;
  }
  topRow.push(<Text key="tr" color={colorAt(pos)}>{TR}</Text>);
  pos++;

  const rightEdgeStart = pos;
  pos += contentRows;

  const brColor = colorAt(pos);
  pos++;
  const bottomHColors: string[] = [];
  for (let i = 0; i < innerW; i++) {
    bottomHColors.push(colorAt(pos));
    pos++;
  }
  const blColor = colorAt(pos);
  pos++;

  const leftEdgeStart = pos;

  const bottomRow: React.ReactNode[] = [];
  bottomRow.push(<Text key="bl" color={blColor}>{BL}</Text>);
  for (let i = innerW - 1; i >= 0; i--) {
    bottomRow.push(<Text key={`b${i}`} color={bottomHColors[i]}>{H}</Text>);
  }
  bottomRow.push(<Text key="br" color={brColor}>{BR}</Text>);

  const leftBorder: React.ReactNode[] = [];
  const rightBorder: React.ReactNode[] = [];
  for (let row = 0; row < contentRows; row++) {
    const lColor = colorAt(leftEdgeStart + (contentRows - 1 - row));
    const rColor = colorAt(rightEdgeStart + row);
    leftBorder.push(<Text key={`l${row}`} color={lColor}>{V}</Text>);
    rightBorder.push(<Text key={`r${row}`} color={rColor}>{V}</Text>);
  }

  return (
    <Box flexDirection="column">
      <Box>{topRow}</Box>
      <Box>
        <Box flexDirection="column">{leftBorder}</Box>
        <Box width={innerW} flexDirection="column" paddingX={1}>
          {children}
        </Box>
        <Box flexDirection="column">{rightBorder}</Box>
      </Box>
      <Box>{bottomRow}</Box>
    </Box>
  );
}
