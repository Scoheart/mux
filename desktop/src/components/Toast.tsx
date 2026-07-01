import { createContext, useContext, useState, useCallback, useRef, ReactNode } from "react";
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

  const show = useCallback(({ kind, msg }: { kind: "success" | "error"; msg: string }) => {
    const id = ++nextId.current;
    setToasts((prev) => [...prev, { id, kind, msg }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 3200);
  }, []);

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
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}
