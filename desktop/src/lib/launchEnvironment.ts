const NAME = /^[A-Za-z_][A-Za-z0-9_]*$/;

export function formatLaunchEnvironment(env: Record<string, string> | undefined): string {
  return Object.entries(env ?? {}).map(([name, value]) => `${name}=${value}`).join("\n");
}

export function parseLaunchEnvironment(text: string): { env: Record<string, string>; error: string | null } {
  const env: Record<string, string> = {};
  const lines = text.split(/\r?\n/);
  let count = 0;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index].trim();
    if (!line || line.startsWith("#")) continue;
    const body = line.replace(/^export\s+/, "");
    const eq = body.indexOf("=");
    if (eq <= 0) return { env: sorted(env), error: `第 ${index + 1} 行需要 NAME=VALUE` };
    const name = body.slice(0, eq).trim();
    let value = body.slice(eq + 1).trim();
    const quote = value[0];
    if (value.length >= 2 && (quote === "\"" || quote === "'") && value.endsWith(quote)) value = value.slice(1, -1);
    count += 1;
    if (count > 64) return { env: sorted(env), error: "环境变量最多 64 项" };
    if (!NAME.test(name) || name.length > 256) return { env: sorted(env), error: "请输入有效变量名，例如 NODE_USE_SYSTEM_CA" };
    if (Object.prototype.hasOwnProperty.call(env, name)) return { env: sorted(env), error: "环境变量名不能重复" };
    if (value.includes("\0") || new TextEncoder().encode(value).length > 8192) return { env: sorted(env), error: "环境变量值无效或过长" };
    env[name] = value;
  }
  return { env: sorted(env), error: null };
}

function sorted(env: Record<string, string>): Record<string, string> {
  return Object.fromEntries(Object.entries(env).sort(([a], [b]) => a.localeCompare(b)));
}
