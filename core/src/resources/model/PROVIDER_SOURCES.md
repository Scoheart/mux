# Provider Catalog sources

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
| `minimax-cn-coding-plan` | `https://api.minimaxi.com/v1` | `https://api.minimaxi.com/anthropic` | <https://platform.minimaxi.com/docs/token-plan/quickstart> |
| `stepfun-step-plan` | `https://api.stepfun.com/step_plan/v1` | `https://api.stepfun.com/step_plan` | <https://platform.stepfun.com/docs/zh/step-plan/quick-start> |
| `stepfun-ai-step-plan` | `https://api.stepfun.ai/step_plan/v1` | `https://api.stepfun.ai/step_plan` | <https://platform.stepfun.ai/docs/en/step-plan/quick-start> |
| `tencent-coding-plan` | `https://api.lkeap.cloud.tencent.com/coding/v3` | `https://api.lkeap.cloud.tencent.com/coding/anthropic` | <https://cloud.tencent.com/document/product/1823/130092> |
| `tencent-token-plan` | `https://api.lkeap.cloud.tencent.com/plan/v3` | `https://api.lkeap.cloud.tencent.com/plan/anthropic` | <https://cloud.tencent.com/document/product/1823/130060> |
| `tencent-token-plan-global` | `https://tokenhub-intl.tencentcloudmaas.com/plan/v3` | `https://tokenhub-intl.tencentcloudmaas.com/plan/anthropic` | <https://intl.cloud.tencent.com/document/product/1300/81315> |

Many plan products are restricted to interactive coding tools and supported
agents. MUX only stores the user-selected connection and credentials locally;
users remain responsible for the vendor's plan eligibility and usage policy.
