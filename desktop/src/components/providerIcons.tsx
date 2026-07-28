const providerIconModules = import.meta.glob("../assets/providers/*.{png,svg,webp}", {
  eager: true,
  query: "?url",
  import: "default",
}) as Record<string, string>;

const PROVIDER_LOGOS = Object.fromEntries(
  Object.entries(providerIconModules).map(([path, url]) => [
    path.split("/").pop()!.replace(/\.[^.]+$/, ""),
    url,
  ]),
) as Record<string, string>;

export const NAMED_PROVIDER_ICON_IDS = [
  "openrouter",
  "anthropic",
  "openai",
  "google",
  "xai",
  "mistral",
  "cohere",
  "deepseek",
  "groq",
  "alibaba",
  "xiaomi",
  "moonshotai",
  "zai",
  "nvidia",
  "cerebras",
  "siliconflow",
  "together",
  "fireworks",
  "poe",
  "huggingface",
  "github-models",
  "novita-ai",
  "qiniu-ai",
  "digitalocean",
  "modelscope",
  "scaleway",
  "nebius",
  "requesty",
  "baseten",
  "wandb",
  "ollama",
  "lm-studio",
  "vllm",
] as const;

const FALLBACK_COLORS = ["#3568D4", "#16856B", "#B84A62", "#9A6618", "#5E55B8", "#277B91"];

function fallbackColor(id: string) {
  let hash = 0;
  for (const char of id) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return FALLBACK_COLORS[hash % FALLBACK_COLORS.length];
}

export function providerIconUrl(id: string): string | undefined {
  return PROVIDER_LOGOS[id];
}

export function ProviderGlyph({
  id,
  name,
  size = 26,
}: {
  id: string;
  name?: string;
  size?: number;
}) {
  const logo = providerIconUrl(id);
  const label = (name || id).trim()[0]?.toUpperCase() ?? "?";
  const radius = Math.round(size * 0.28);

  return (
    <span
      aria-hidden="true"
      data-provider-icon={logo ? id : "fallback"}
      className="mux-provider-glyph"
      style={{
        width: size,
        height: size,
        borderRadius: radius,
        background: logo ? "#fff" : fallbackColor(id),
      }}
    >
      <span className="mux-provider-glyph-fallback" style={{ fontSize: Math.round(size * 0.46) }}>
        {label}
      </span>
      {logo && (
        <img
          src={logo}
          alt=""
          draggable={false}
          onError={(event) => { event.currentTarget.style.display = "none"; }}
        />
      )}
    </span>
  );
}
