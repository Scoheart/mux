import { useEffect } from "react";
import { cliStatus, installCli } from "../lib/api";
import { useToast } from "../components/Toast";

// One-time flags so we don't re-toast on every launch.
const INSTALLED_KEY = "mux-cli-installed-notified";
const PATH_HINT_KEY = "mux-cli-path-hint-notified";

/**
 * 桌面 App 自带 mux CLI（sidecar）：启动后静默把它软链到 ~/.local/bin/mux。
 * 软链指向包内 → App 自动更新后 CLI 同步更新，且断链（App 挪位置）会在下次
 * 启动时自动修复。只在首次安装成功 / PATH 缺失时各提示一次。
 */
export function useCliTool() {
  const toast = useToast();

  useEffect(() => {
    const t = setTimeout(async () => {
      try {
        const status = await cliStatus();
        if (!status.bundled) return; // dev 构建没有 sidecar
        let inPath = status.in_path;
        if (!status.installed) {
          const after = await installCli();
          inPath = after.in_path;
          if (localStorage.getItem(INSTALLED_KEY) !== "1") {
            localStorage.setItem(INSTALLED_KEY, "1");
            toast.show({ kind: "success", msg: `命令行工具已就绪：终端里直接运行 mux（${after.link_path}）` });
          }
        }
        if (!inPath && localStorage.getItem(PATH_HINT_KEY) !== "1") {
          localStorage.setItem(PATH_HINT_KEY, "1");
          toast.show({
            kind: "error",
            msg: "mux CLI 已安装到 ~/.local/bin，但该目录不在 PATH — 在 shell 配置里加入：export PATH=\"$HOME/.local/bin:$PATH\"",
          });
        }
      } catch {
        // 静默失败：CLI 安装是锦上添花，不该打断启动。
      }
    }, 3500);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
