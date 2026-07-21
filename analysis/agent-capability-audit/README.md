# MUX Agent MCP / Models / Skills 全量审计

> 状态：已完成。核验日期：2026-07-22；研究证据、实现边界与最终统计均已收口。

## 审计目标与口径

审计集合取审计开始时的 `data/agents.json`、同步后的 `data/agent-catalog.json` 与 ACP Registry 固定 commit manifest 的归一化 ID 并集。最终口径为：

- 审计开始时：45 个 audited、175 个 discovery catalog、24 个重叠身份，即 196 个 MUX 内去重身份；
- ACP Registry 当前 39 个 manifest 归一化后带来 15 个此前缺失身份；
- 2026-07-22 同步 Glama 与 ACP Registry 后，catalog 为 201 条；与审计开始时 45 个 audited 定义去重后，本轮总审计集合为 211 个身份；
- 本版实现后 registry 为 56 个 audited definitions，其中 46 个 MCP writer、45 个 Skills target；Model 共有 14 个 target（12 managed、2 guided）。

每个身份必须分别回答以下问题，不能由一类资产推断另一类：

1. 官方身份、别名、客户端形态和可验证安装探针；
2. MCP 的用户级与项目级路径、格式、顶层键、条目 layout、stdio / HTTP 字段和优先级；
3. Models 的配置路径、provider/model schema、多模型与当前模型语义、对话内切换方式、Base URL 与凭据引用方式；
4. Skills 的用户级与项目级目录、兼容读取目录、发现优先级以及是否允许修改；
5. 是否存在稳定用户级可写契约，是否符合 MUX 的无密钥、保真、fail-closed 边界；
6. 官方文档、官方 GitHub 源码或官方发布包证据，以及社区线索和冲突说明。

## 证据等级

| 等级 | 含义 | 能否开启 MUX writer |
|---|---|---|
| `official-runtime` | 官方源码中实际读取/写入路径与 schema | 可以，仍需 fixture 与保真测试 |
| `official-docs` | 官方文档明确给出路径与结构 | 可以，需测试覆盖歧义与版本差异 |
| `official-package` | 官方签名发布包或发布制品中可复现 | 可以，需记录版本 |
| `community` | 社区教程、目录或讨论 | 不可以，只作官方证据检索线索 |
| `not-found` | 已完成约定检索但未发现稳定契约 | 不可以，保留 discovery-only |
| `conflict` | 官方来源互相冲突、随工作区/环境变化或有多个活动配置 | 不可以，先设计显式选择/冲突处理 |

## 增量产物

- `catalog-source-discovery.json`：从 catalog 页面机器提取的描述、官方站点和 GitHub repo 候选；每个 catalog ID 都有一条记录。
- `acp-registry-source-discovery.json`：从 ACP Registry 当前 commit 获取的 Agent manifest、版本、repo 与分发方式；ACP 只作身份/启动证据。
- `repository-tree-scans.json`：对已识别 GitHub repo 的固定 commit tree 扫描结果，按 MCP、Models、Skills、config 路径分类；它用于定位源码，不单独证明配置语义。
- `repository-evidence-snippets.json`：按 Git blob SHA 下载高信号文件并提取路径、schema、provider/model、凭据引用与 Skills 相关上下文；公开示例凭据会在落盘前脱敏。
- `verified-agents-evidence.md`：审计开始时 45 个 audited Agent 的人工证据复核。
- `catalog-a-m-evidence.md` / `catalog-n-z-evidence.md`：审计开始时 catalog-only / ACP 新身份的逐项证据复核；本版提升项仍保留在原分片中，便于追溯其研究来源。
- `implementation-gap.md`：证据与 registry / codec / Models / Skills / probe 的差异和改造决策。
- `final-report.md`：汇总覆盖率、实现统计、本版交付与仍未找到安全契约的 Agent。

## 机器校验边界

- `node scripts/validate-agent-capability-audit.mjs` 只证明 211 个规范化身份在三个证据分片中各出现一次，不证明 writer/target 实现数量。
- `coverage-baseline.md` / `coverage-baseline.json` 从最终工作树生成，用于证明 56 audited、46 MCP writer、45 Skills target、14 Model target（12 managed、2 guided）等实现统计。

## 重新生成与验证

```bash
MUX_AUDIT_DATE=2026-07-22 node scripts/discover-agent-catalog-sources.mjs
node scripts/discover-acp-registry-agents.mjs
node scripts/scan-agent-repository-trees.mjs
node --test scripts/extract-agent-repository-evidence.test.mjs
node scripts/extract-agent-repository-evidence.mjs
node scripts/build-agent-capability-baseline.mjs
node scripts/validate-agent-capability-audit.mjs
```

Repository tree、blob URL 与人工报告中的 GitHub 字段证据应固定到 commit。`coverage-baseline.json` 的 `currentDocs`、ACP manifest 的 `website` 和源码 snippet 中的 URL 是上游原始元数据，不作为固定字段语义的研究证明；对应证明由 pinned `blobUrl`、`manifestUrl` 和 Markdown 证据链接承担。证据抽取必须先通过 redaction tests，再对 redacted payload 与最终 serialized JSON 两阶段执行 credential-shaped fail-closed 检查。

## 写入门禁

- Glama、MCP client lists、ACP Registry 和社区文章只能证明“有人列出/可启动”，不能证明本地配置契约。
- 没有稳定用户级路径时，不把项目文件或当前目录文件伪装成全局配置。
- API Key / token 只能通过 Keychain 或官方环境变量引用；官方只能明文落盘的功能保持 guided/read-only。
- 多 Home、多 profile 或运行参数决定当前配置时，必须先实现显式环境选择，不能硬编码默认路径后宣称全量支持。
- 搜索无结果也是结论，但必须记录查询范围和 `not-found`，不能空白。
