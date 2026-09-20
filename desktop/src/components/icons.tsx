import { CSSProperties } from "react";

interface IconProps { className?: string; style?: CSSProperties; }

/** Official MCP mark: three interlocking strokes, kept as a line icon so it
 * remains crisp in the compact navigation and resource tabs. */
export function McpMarkIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 190 190" className={className} style={style} fill="none" aria-hidden="true">
      <path d="M18 94 69 43c7-7 18-7 25 0s7 18 0 25L56 106" stroke="currentColor" strokeWidth="12" strokeLinecap="round" />
      <path d="m57 105 38-38c7-7 18-7 25 0l.3.3c7 7 7 18 0 25l-46 46c-2.5 2.5-2.5 6.5 0 9l9.5 9.5" stroke="currentColor" strokeWidth="12" strokeLinecap="round" />
      <path d="m82 56-38 38c-7 7-7 18 0 25s18 7 25 0l38-38" stroke="currentColor" strokeWidth="12" strokeLinecap="round" />
    </svg>
  );
}

/** Agent Skills mark: a restrained hexagonal badge with an inner capability
 * node, matching the official site's geometric favicon at small sizes. */
export function SkillMarkIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="m12 2.2 8.5 4.9v9.8L12 21.8l-8.5-4.9V7.1z" />
      <path d="m8.3 9.2 3.7-2.1 3.7 2.1v5.6L12 16.9l-3.7-2.1z" />
      <path d="M12 7.1v9.8M8.3 9.2l7.4 4.2M15.7 9.2l-7.4 4.2" opacity=".6" />
    </svg>
  );
}

/** Brain mark for model resources. The two lobes and central stem stay
 * recognizable at 14–24px without turning the tab into a filled glyph. */
export function BrainIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M12 20.5V5.1a3 3 0 0 0-5.6-1.5A3.2 3.2 0 0 0 3.5 6.5c0 .5.1 1 .3 1.4A3.6 3.6 0 0 0 4.8 15a3.1 3.1 0 0 0 4.6 3.7A3 3 0 0 0 12 20.5Z" />
      <path d="M12 20.5V5.1a3 3 0 0 1 5.6-1.5 3.2 3.2 0 0 1 2.9 2.9c0 .5-.1 1-.3 1.4a3.6 3.6 0 0 1-1 7.1 3.1 3.1 0 0 1-4.6 3.7A3 3 0 0 1 12 20.5Z" />
      <path d="M8.3 7.1c1 .2 1.7 1 1.7 2M6.1 11.2c1 .1 1.8.7 2.2 1.6M15.7 7.1c-1 .2-1.7 1-1.7 2M17.9 11.2c-1 .1-1.8.7-2.2 1.6" />
    </svg>
  );
}

export function SlidersIcon({ className, style }: IconProps) {
  return <svg viewBox="0 0 24 24" className={className} style={style} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
    <path d="M6 3v5m0 4v9M12 3v10m0 4v4M18 3v2m0 4v12M3 8h6m0 5h6m0-8h6" />
  </svg>;
}

export function PlayIcon({ className, style }: IconProps) {
  return <svg viewBox="0 0 24 24" className={className} style={style} fill="currentColor" aria-hidden="true">
    <path d="M8 4.5a1 1 0 0 0-1.5.86v13.28a1 1 0 0 0 1.5.86l11-6.64a1 1 0 0 0 0-1.72Z" />
  </svg>;
}

export function DocumentIcon({ className, style }: IconProps) {
  return <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
    strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z" />
    <path d="M14 2v6h6M8 13h8M8 17h5" />
  </svg>;
}

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

export function NetworkIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="2.5" />
      <circle cx="5" cy="6" r="2" />
      <circle cx="19" cy="6" r="2" />
      <circle cx="12" cy="20" r="2" />
      <path d="m6.6 7.2 3.5 3M17.4 7.2l-3.5 3M12 14.5V18" />
    </svg>
  );
}

export function LanguageIcon({ className, style }: IconProps) {
  return (
    <svg className={className} style={style} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="9" />
      <path d="M3.5 9h17M3.5 15h17M12 3c2.2 2.4 3.3 5.4 3.3 9S14.2 18.6 12 21c-2.2-2.4-3.3-5.4-3.3-9S9.8 5.4 12 3Z" />
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

export function SparklesIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="m12 3-1.9 5.8a2 2 0 0 1-1.3 1.3L3 12l5.8 1.9a2 2 0 0 1 1.3 1.3L12 21l1.9-5.8a2 2 0 0 1 1.3-1.3L21 12l-5.8-1.9a2 2 0 0 1-1.3-1.3Z" />
      <path d="M5 3v4M3 5h4M19 17v4M17 19h4" />
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

export function KeyIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="8" cy="15" r="4" />
      <path d="m11 12 8-8m-3 3 2 2m-5 1 2 2" />
    </svg>
  );
}

export function EyeIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6Z" />
      <circle cx="12" cy="12" r="2.7" />
    </svg>
  );
}

export function EyeOffIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="m3 3 18 18" />
      <path d="M10.5 6.2A10.8 10.8 0 0 1 12 6c6 0 9.5 6 9.5 6a15.8 15.8 0 0 1-2.2 2.9M6.1 6.1C3.8 7.8 2.5 12 2.5 12s3.5 6 9.5 6c1.7 0 3.2-.5 4.5-1.2" />
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

export function ExternalLinkIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M14 3h7v7" />
      <path d="M10 14 21 3" />
      <path d="M21 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h6" />
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

export function PinIcon({ className, style }: IconProps) {
  return (
    <svg data-icon="pin" viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 17v5" />
      <path d="M5 17h14" />
      <path d="M7 3h10l-1 6 3 3v2H5v-2l3-3Z" />
    </svg>
  );
}

export function PinOffIcon({ className, style }: IconProps) {
  return (
    <svg data-icon="pin-off" viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 17v5" />
      <path d="M5 17h14" />
      <path d="M7 3h10l-1 6 3 3v2H5v-2l3-3Z" />
      <path d="M3 3l18 18" />
    </svg>
  );
}

export function GripVerticalIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="9" cy="6" r="1" fill="currentColor" stroke="none" />
      <circle cx="15" cy="6" r="1" fill="currentColor" stroke="none" />
      <circle cx="9" cy="12" r="1" fill="currentColor" stroke="none" />
      <circle cx="15" cy="12" r="1" fill="currentColor" stroke="none" />
      <circle cx="9" cy="18" r="1" fill="currentColor" stroke="none" />
      <circle cx="15" cy="18" r="1" fill="currentColor" stroke="none" />
    </svg>
  );
}

export function MoreHorizontalIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="5" cy="12" r="1" fill="currentColor" stroke="none" />
      <circle cx="12" cy="12" r="1" fill="currentColor" stroke="none" />
      <circle cx="19" cy="12" r="1" fill="currentColor" stroke="none" />
    </svg>
  );
}

export function GaugeIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4.9 19a9 9 0 1 1 14.2 0" />
      <path d="m12 13 4-4" />
      <path d="M12 19h.01" />
    </svg>
  );
}

export function CalendarIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="5" width="18" height="16" rx="2" />
      <path d="M16 3v4M8 3v4M3 10h18" />
    </svg>
  );
}

export function LinkOffIcon({ className, style }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" className={className} style={style} stroke="currentColor" fill="none"
      strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="m3 3 18 18" />
      <path d="M10.6 13.4a5 5 0 0 0 6.9.1l3-3a5 5 0 0 0-6.9-7.2l-1.7 1.6" />
      <path d="M13.4 10.6a5 5 0 0 0-6.9-.1l-3 3a5 5 0 0 0 6.9 7.2l1.7-1.6" />
    </svg>
  );
}

/** Half-filled circle (◐) to indicate customized/overridden state */
