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
  const [leaving, setLeaving] = useState(false);
  const nextId = useRef(0);
  const active = toasts[0];
  const dismiss = useCallback(() => setLeaving(true), []);

  const show = useCallback(({ kind, msg }: { kind: "success" | "error"; msg: string }) => {
    const id = ++nextId.current;
    // Repeated failures should not turn into a long sequence of identical cards.
    setToasts((prev) => prev.some((toast) => toast.kind === kind && toast.msg === msg)
      ? prev : [...prev, { id, kind, msg }]);
  }, []);

  useEffect(() => {
    if (!active || leaving) return;
    const timer = setTimeout(dismiss, active.kind === "error" ? 9000 : 6000);
    return () => clearTimeout(timer);
  }, [active?.id, active?.kind, leaving, dismiss]);

  useEffect(() => {
    if (!leaving) return;
    const timer = setTimeout(() => {
      setToasts((current) => current.slice(1));
      setLeaving(false);
    }, 180);
    return () => clearTimeout(timer);
  }, [leaving]);

  return (
    <ToastContext.Provider value={{ show }}>
      {children}
      {active && (
        <div className="mux-toast-viewport">
          <div key={active.id} className="mux-toast" data-leaving={leaving || undefined}
            role={active.kind === "error" ? "alert" : "status"} aria-atomic="true">
            <div className="mux-toast-icon" data-kind={active.kind}>
              {active.kind === "success" ? <CheckIcon className="w-3 h-3 text-white" /> : <XIcon className="w-3 h-3 text-white" />}
            </div>
            <span className="mux-toast-message">{active.msg}</span>
            <button type="button" className="mux-icon-btn mux-toast-close"
              aria-label={`关闭${active.kind === "error" ? "错误" : "成功"}通知`}
              title="关闭通知" onClick={dismiss}><XIcon className="w-3.5 h-3.5" /></button>
          </div>
        </div>
      )}
    </ToastContext.Provider>
  );
}
