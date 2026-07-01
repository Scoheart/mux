import { homedir } from "node:os";
import { resolve, isAbsolute } from "node:path";

export function expandTilde(filePath: string): string {
  if (filePath.startsWith("~/")) {
    return resolve(homedir(), filePath.slice(2));
  }
  return filePath;
}

export function resolvePath(
  configPath: string,
  scope: "global" | "project",
  projectDir?: string
): string {
  if (scope === "global") {
    return expandTilde(configPath);
  }
  if (!projectDir) {
    throw new Error(`Project directory required for project-scope path: ${configPath}`);
  }
  if (isAbsolute(configPath)) {
    return configPath;
  }
  return resolve(projectDir, configPath);
}
