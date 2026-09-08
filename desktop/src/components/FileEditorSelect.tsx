import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { formatError } from "../lib/format";
import { useToast } from "./Toast";

import { AgentGlyph } from "./brandIcons";
import { ChevronDownIcon, EditIcon, FolderIcon } from "./icons";

const STORAGE_KEY = "mux.file-editor";
const EDITORS = ["Visual Studio Code", "Cursor", "Sublime Text", "Zed", "TextEdit"];
const ICONS: Record<string, string> = { "Visual Studio Code": "vscode", Cursor: "cursor", Zed: "zed", Qoder: "qoder", "Qoder IDE": "qoder", Windsurf: "windsurf", Antigravity: "antigravity", ZCode: "zcode" };
function EditorIcon({ editor }: { editor: string }) {
  editor = editorLabel(editor);
  if (ICONS[editor]) return <AgentGlyph id={ICONS[editor]} size={20} />;
  if (editor === "Sublime Text") return <span className="mux-editor-sublime" aria-hidden="true">S</span>;
  return editor ? <EditIcon className="w-4 h-4" /> : <FolderIcon className="w-4 h-4" />;
}

export function preferredFileEditor(): string | undefined {
  try {
    return localStorage.getItem(STORAGE_KEY) || undefined;
  } catch {
    return undefined;
  }
}

function editorLabel(editor: string): string {
  return editor.split(/[\\/]/).pop()?.replace(/\.app$/i, "") || editor;
}

export function FileEditorSelect() {
  const [editor, setEditor] = useState(() => preferredFileEditor() ?? "");
  const [installed, setInstalled] = useState<Array<{name: string; path: string}>>([]);
  const [choosing, setChoosing] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!expanded) return;
    void invoke<Array<{name: string; path: string}>>("list_file_editors")
      .then(setInstalled).catch(() => {});
    root.current?.querySelector<HTMLButtonElement>('[role="menuitemradio"][aria-checked="true"]')?.focus();
    const dismiss = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setExpanded(false);
    };
    document.addEventListener("pointerdown", dismiss);
    return () => document.removeEventListener("pointerdown", dismiss);
  }, [expanded]);
  const { show: showToast } = useToast();

  async function change(value: string) {
    setExpanded(false);
    trigger.current?.focus();
    setChoosing(true);
    try {
      let selected = value;
      if (value === "__choose__") {
        const path = await open({
          title: "选择用于打开文件的编辑器",
          defaultPath: "/Applications",
          filters: [{ name: "应用程序", extensions: ["app"] }],
          multiple: false,
          directory: false,
        });
        if (!path) return;
        selected = path;
      }
      if (selected) localStorage.setItem(STORAGE_KEY, selected);
      else localStorage.removeItem(STORAGE_KEY);
      setEditor(selected);
      showToast({ kind: "success", msg: selected ? `文件将使用 ${editorLabel(selected)} 打开` : "已恢复系统默认应用" });
    } catch (error) {
      showToast({ kind: "error", msg: `无法保存编辑器：${formatError(error)}` });
    } finally {
      setChoosing(false);
    }
  }

  const available = installed.length ? installed.map((item) => item.path) : EDITORS;
  const options = [...available, ...(editor && !available.includes(editor) ? [editor] : []), ""];
  return (
    <div className="mux-editor-menu-wrap" ref={root}
      onBlur={(event) => {
        // Safari can report null when clicking a button. Do not unmount it before click.
        if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget)) setExpanded(false);
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape") { event.preventDefault(); setExpanded(false); trigger.current?.focus(); }
        if (expanded && ["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
          event.preventDefault();
          const items = Array.from(root.current?.querySelectorAll<HTMLButtonElement>('[role^="menuitem"]') ?? []);
          const index = items.indexOf(document.activeElement as HTMLButtonElement);
          const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : (index + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
          items[next]?.focus();
        }
      }}>
      <button type="button" ref={trigger} className="mux-editor-trigger"
        aria-label={`文件编辑器：${editor ? editorLabel(editor) : "系统默认"}`}
        title="选择打开文件的编辑器" aria-haspopup="menu" aria-expanded={expanded}
        disabled={choosing} onClick={() => setExpanded((value) => !value)}
        onKeyDown={(event) => { if (!expanded && event.key === "ArrowDown") { event.preventDefault(); setExpanded(true); } }}>
        <EditorIcon editor={editor} /><ChevronDownIcon className="w-3 h-3" />
      </button>
      {expanded && <div className="mux-editor-menu" role="menu" aria-label="打开文件的编辑器">
        {options.map((value) => <button type="button" role="menuitemradio" aria-checked={value === editor}
          onMouseDown={(event) => event.preventDefault()}
          key={value || "default"} onClick={() => { void change(value); }}>
          <EditorIcon editor={value} />
          <span>{value === "Visual Studio Code" ? "VS Code" : value ? editorLabel(value) : "系统默认应用"}</span>
          {value === editor && <span className="mux-editor-check" aria-hidden="true">✓</span>}
        </button>)}
        <div role="separator" className="mux-editor-separator" />
        <button type="button" role="menuitem" onMouseDown={(event) => event.preventDefault()} onClick={() => { void change("__choose__"); }}>
          <EditIcon className="w-4 h-4" /><span>选择其他应用…</span>
        </button>
      </div>}
    </div>
  );
}
