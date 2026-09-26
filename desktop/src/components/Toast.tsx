import { createContext, useContext, useState, useCallback, useEffect, useRef, ReactNode } from "react";
import { CheckIcon, XIcon } from "./icons";

interface ToastItem {
  id: number;
  kind: "success" | "error";
  msg: string;
}

interface ToastContextValue {
  show: (toast: { kind: "success" | "error"; msg: string }) => void;
}

const ToastContext = createContext<ToastContextValue>({ show: () => {} });

export function useToast() {
  return useContext(ToastContext);
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const nextId = useRef(0);
  const timers = useRef(new Map<number, ReturnType<typeof setTimeout>>());
  const dismiss = useCallback((id: number) => {
    clearTimeout(timers.current.get(id));
    timers.current.delete(id);
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);
  useEffect(() => () => {
    timers.current.forEach(clearTimeout);
    timers.current.clear();
  }, []);

  const show = useCallback(({ kind, msg }: { kind: "success" | "error"; msg: string }) => {
    const id = ++nextId.current;
    setToasts((prev) => [...prev, { id, kind, msg }]);
    // Failures stay available for inspection; successful operations are quiet
    // after a readable interval. Neither needs polling to infer completion.
    if (kind === "success") timers.current.set(id, setTimeout(() => dismiss(id), 6000));
  }, [dismiss]);

  return (
    <ToastContext.Provider value={{ show }}>
      {children}
      {/* Toast container */}
      <div
        className="fixed bottom-5 left-1/2 -translate-x-1/2 flex flex-col gap-2 z-50"
        style={{ pointerEvents: "none" }}
      >
        {toasts.map((t) => (
          <div
            key={t.id}
            role={t.kind === "error" ? "alert" : "status"}
            aria-atomic="true"
            className="flex items-center gap-2.5 px-4 py-3 rounded-mac text-sm font-medium"
            style={{
              background: "var(--glass-fill-strong)",
              backdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
              WebkitBackdropFilter: "blur(var(--glass-blur)) saturate(var(--glass-saturate))",
              border: "1px solid var(--glass-border)",
              boxShadow: "var(--shadow-sheet), var(--glass-highlight)",
              color: "var(--text-primary)",
              pointerEvents: "auto",
              minWidth: 220,
            }}
          >
            <div
              className="w-5 h-5 rounded-full flex items-center justify-center flex-shrink-0"
              style={{
                background: t.kind === "success" ? "#34C759" : "#FF3B30",
              }}
            >
              {t.kind === "success" ? (
                <CheckIcon className="w-3 h-3 text-white" />
              ) : (
                <XIcon className="w-3 h-3 text-white" />
              )}
            </div>
            <span>{t.msg}</span>
            <button type="button" className="mux-icon-btn" aria-label={`关闭${t.kind === "error" ? "错误" : "成功"}通知 ${t.id}`}
              title="关闭通知" onClick={() => dismiss(t.id)}><XIcon className="w-3.5 h-3.5" /></button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}
