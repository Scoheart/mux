# Provider Catalog sources

## Clickable official documentation — 2026-09-21

`data/provider-docs.json` is the display-only documentation URL catalog for all 71 built-in Provider templates. Core serializes it as `docs_url`; custom connections have no vendor documentation link. It is independent of user-supplied API endpoints and never derives a website from a Base URL or includes credentials.

The desktop exposes the same link in the selected Provider banner, the Provider picker selection footer, and the Provider editor footer. Links open via the native system-browser opener without selecting a template or submitting a form; failures use the existing toast UI.

URLs were checked against the official pages on this date, using the existing source matrices and models.dev for discovery only. Regions and Coding/Token Plans retain their specific documentation. Direct requests to some vendors encountered network-policy or anti-bot pages; official page retrieval separately confirmed MiniMax CN, Kimi Code, Requesty, Together AI, Scaleway and StepFun Global. Network interception URLs are not stored in the catalog. MiMo Token Plan uses the current `/tokenplan/Token%20Plan/quick-access` route rather than the outdated `/price/tokenplan/quick-access` route.

Plan-specific Provider templates were audited on 2026-07-28 against
`https://models.dev/api.json` for discovery and the vendors' official
documentation for authority. A Coding/Token Plan is a separate connection:
its API key and endpoint must not be mixed with the vendor's pay-as-you-go API.

| MUX Provider ID | OpenAI-compatible Base URL | Anthropic-compatible Base URL | Official source |
|---|---|---|---|
| `zai-coding-plan` | `https://api.z.ai/api/coding/paas/v4` | `https://api.z.ai/api/anthropic` | <https://docs.z.ai/devpack/quick-start> |
| `zhipuai-coding-plan` | `https://open.bigmodel.cn/api/coding/paas/v4` | `https://open.bigmodel.cn/api/anthropic` | <https://docs.bigmodel.cn/cn/coding-plan/quick-start> |
| `alibaba-coding-plan-cn` | `https://coding.dashscope.aliyuncs.com/v1` | `https://coding.dashscope.aliyuncs.com/apps/anthropic` | <https://help.aliyun.com/zh/model-studio/coding-plan> |
| `alibaba-coding-plan` | `https://coding-intl.dashscope.aliyuncs.com/v1` | `https://coding-intl.dashscope.aliyuncs.com/apps/anthropic` | <https://www.alibabacloud.com/help/en/model-studio/coding-plan> |
| `alibaba-token-plan-cn` | `https://token-plan.cn-beijing.maas.aliyuncs.com/compatible-mode/v1` | `https://token-plan.cn-beijing.maas.aliyuncs.com/apps/anthropic` | <https://help.aliyun.com/zh/model-studio/token-plan-personal-quick-start> |
| `alibaba-token-plan` | `https://token-plan.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1` | `https://token-plan.ap-southeast-1.maas.aliyuncs.com/apps/anthropic` | <https://www.alibabacloud.com/help/en/model-studio/token-plan-quickstart> |
| `xiaomi-token-plan-cn` | `https://token-plan-cn.xiaomimimo.com/v1` | `https://token-plan-cn.xiaomimimo.com/anthropic` | <https://mimo.mi.com/docs/zh-CN/price/tokenplan/quick-access> |
| `xiaomi-token-plan-sgp` | `https://token-plan-sgp.xiaomimimo.com/v1` | `https://token-plan-sgp.xiaomimimo.com/anthropic` | <https://mimo.mi.com/docs/zh-CN/price/tokenplan/quick-access> |
| `xiaomi-token-plan-ams` | `https://token-plan-ams.xiaomimimo.com/v1` | `https://token-plan-ams.xiaomimimo.com/anthropic` | <https://mimo.mi.com/docs/zh-CN/price/tokenplan/quick-access> |
| `kimi-for-coding` | `https://api.kimi.com/coding/v1` | `https://api.kimi.com/coding` | <https://www.kimi.com/code/docs/en/> |
| `minimax-coding-plan` | `https://api.minimax.io/v1` | `https://api.minimax.io/anthropic` | <https://platform.minimax.io/docs/token-plan/quickstart> |
| `minimax-cn-coding-plan` | `https://api.minimax.cn/v1` | `https://api.minimax.cn/anthropic` | <https://platform.minimaxi.com/docs/token-plan/quickstart> |
| `stepfun-step-plan` | `https://api.stepfun.com/step_plan/v1` | `https://api.stepfun.com/step_plan` | <https://platform.stepfun.com/docs/zh/step-plan/quick-start> |
| `stepfun-ai-step-plan` | `https://api.stepfun.ai/step_plan/v1` | `https://api.stepfun.ai/step_plan` | <https://platform.stepfun.ai/docs/en/step-plan/quick-start> |
| `tencent-coding-plan` | `https://api.lkeap.cloud.tencent.com/coding/v3` | `https://api.lkeap.cloud.tencent.com/coding/anthropic` | <https://cloud.tencent.com/document/product/1823/130092> |
| `tencent-token-plan` | `https://api.lkeap.cloud.tencent.com/plan/v3` | `https://api.lkeap.cloud.tencent.com/plan/anthropic` | <https://cloud.tencent.com/document/product/1823/130060> |
| `tencent-token-plan-global` | `https://tokenhub-intl.tencentcloudmaas.com/plan/v3` | `https://tokenhub-intl.tencentcloudmaas.com/plan/anthropic` | <https://intl.cloud.tencent.com/document/product/1300/81315> |

Many plan products are restricted to interactive coding tools and supported
agents. MUX only stores the user-selected connection and credentials locally;
users remain responsible for the vendor's plan eligibility and usage policy.

The China MiniMax template endpoints were reverified on 2026-09-07 against the current official Token Plan and OpenAI/Anthropic SDK guides (`api.minimax.cn`). Existing saved connections are not rewritten.

The September 2026 expansion adds 19 templates and removes the retired GitHub Models new-connection template. See the [full endpoint/source matrix](../../../../docs/agent-provider-research-2026-09-07.md). Shared MiniMax PAYG/plan addresses do not identify billing mode; explicit plan selections are preserved.

## Gateway Provider templates

These gateway templates were reviewed on 2026-09-20. They are selectable
first-class MUX Providers, not aliases for the underlying model vendors. MUX
keeps the OpenAI Responses/Completions and Anthropic Messages paths as separate
protocol entries so one Provider can be reused by Agents with different
protocol requirements.

| MUX Provider ID | OpenAI-compatible Base URL | Anthropic-compatible Base URL | Official source |
|---|---|---|---|
| `routeway` | `https://api.routeway.ai/v1` | `https://api.routeway.ai` | [Codex integration](https://docs.routeway.ai/integrations/agents/codex), [Claude Code integration](https://docs.routeway.ai/integrations/agents/claude-code) |
| `infron` | `https://llm.onerouter.pro/v1` | `https://llm.onerouter.pro` | [API reference](https://infron.ai/models/deepseek/deepseek-v4-flash:free/api-reference) |
| `apinex` | `https://api.apinex.bond/v1` | `https://api.apinex.bond` | [Chat API](https://apinex.bond/developers/models/chat), [Messages API](https://apinex.bond/developers/models/messages) |
| `tokenharbor` | `https://tokenharbor.ai/v1` | `https://tokenharbor.ai` | [Chat and Messages](https://tokenharbor.ai/docs/api/curl), [Codex integration](https://tokenharbor.ai/docs/integrations/codex) |

APInex is currently exposed with Chat Completions and Anthropic Messages only;
its public documentation does not establish a separate Responses endpoint.
Model IDs remain dynamic and are loaded from each Provider's `/models` catalog
when available rather than being hard-coded from social media posts.
