import { CSSProperties } from "react";

interface IconProps { className?: string; style?: CSSProperties; }

export function SearchIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="11" cy="11" r="8" />
      <line x1="21" y1="21" x2="16.65" y2="16.65" />
    </svg>
  );
}

export function ChevronDownIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="6 9 12 15 18 9" />
    </svg>
  );
}

export function SunIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
    </svg>
  );
}

export function CloudIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M20 17.58A5 5 0 0 0 18 8h-1.26A8 8 0 1 0 4 16.25" />
      <polyline points="8 16 12 12 16 16" />
      <line x1="12" y1="12" x2="12" y2="21" />
    </svg>
  );
}

export function TrashIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="3 6 5 6 21 6" />
      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
      <line x1="10" y1="11" x2="10" y2="17" />
      <line x1="14" y1="11" x2="14" y2="17" />
    </svg>
  );
}

export function MoonIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
    </svg>
  );
}

export function FolderIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
    </svg>
  );
}

export function CheckIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="20 6 9 17 4 12" />
    </svg>
  );
}

export function PackageIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <line x1="16.5" y1="9.4" x2="7.5" y2="4.21" />
      <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
      <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
      <line x1="12" y1="22.08" x2="12" y2="12" />
    </svg>
  );
}

/** Bot / agent mark — rounded head, antenna, ear stubs */
export function BotIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 8V4h4" />
      <rect width="16" height="12" x="4" y="8" rx="3" />
      <path d="M2 14h2" />
      <path d="M20 14h2" />
      <path d="M9 13v2" />
      <path d="M15 13v2" />
    </svg>
  );
}

/** Wrench — MCP / tools tab */
export function WrenchIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
    </svg>
  );
}

export function LayersIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polygon points="12 2 2 7 12 12 22 7 12 2" />
      <polyline points="2 17 12 22 22 17" />
      <polyline points="2 12 12 17 22 12" />
    </svg>
  );
}

/** Brain — Models tab (compact lucide-style mark, same optical size as wrench) */
export function BrainIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M9.5 2A2.5 2.5 0 0 1 12 4.5v15a2.5 2.5 0 0 1-4.96.44 2.5 2.5 0 0 1-2.96-3.08 3 3 0 0 1-.34-5.58 2.5 2.5 0 0 1 1.32-4.24 2.5 2.5 0 0 1 4.44-1.54Z" />
      <path d="M14.5 2A2.5 2.5 0 0 0 12 4.5v15a2.5 2.5 0 0 0 4.96.44 2.5 2.5 0 0 0 2.96-3.08 3 3 0 0 0 .34-5.58 2.5 2.5 0 0 0-1.32-4.24 2.5 2.5 0 0 0-4.44-1.54Z" />
    </svg>
  );
}

export function PlusIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <line x1="12" y1="5" x2="12" y2="19" />
      <line x1="5" y1="12" x2="19" y2="12" />
    </svg>
  );
}

export function XIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <line x1="18" y1="6" x2="6" y2="18" />
      <line x1="6" y1="6" x2="18" y2="18" />
    </svg>
  );
}

/** Two columns of dots — drag handle affordance */
export function CopyIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="9" y="9" width="13" height="13" rx="2" />
      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
    </svg>
  );
}

export function RefreshIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="23 4 23 10 17 10" />
      <polyline points="1 20 1 14 7 14" />
      <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
    </svg>
  );
}

export function LinkIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
      <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
    </svg>
  );
}

export function ArrowLeftIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <line x1="19" y1="12" x2="5" y2="12" />
      <polyline points="12 19 5 12 12 5" />
    </svg>
  );
}

export function SaveIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2Z" />
      <polyline points="17 21 17 13 7 13 7 21" />
      <polyline points="7 3 7 8 15 8" />
    </svg>
  );
}

export function EditIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" />
    </svg>
  );
}

export function DownloadIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
      <polyline points="7 10 12 15 17 10" />
      <line x1="12" y1="15" x2="12" y2="3" />
    </svg>
  );
}

export function TerminalIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="4 17 10 11 4 5" />
      <line x1="12" y1="19" x2="20" y2="19" />
    </svg>
  );
}

/** Half-filled circle (◐) to indicate customized/overridden state */
