# MUX 的 Computer Use 可操作性优化

分析日期：2026-09-27。范围：正式桌面端的 Agent 导航、资源查找、弹窗和状态反馈。

## 结论

MUX 可以减少自动化操作中的查找、试点和装饰动画等待。关键是让页面暴露准确的控件名称、焦点、当前状态和键盘路径，而不是给某一家 Agent 加专用控制接口。本次改动集中在 React 公共组件，资源写入仍沿用 Rust core 的计划、确认和提交流程。

这些优化降低操作步骤与歧义；没有测量端到端耗时百分比，也不能据此推断 Codex 或其他 Agent 的内部推理速度。不同 Computer Use 工具是否利用可访问性树、快捷键或仅分析截图，取决于各自实现。

## 操作链路

```mermaid
flowchart LR
    A[Agent 观察窗口] --> B[截图或系统可访问性树]
    B --> C[名称、状态与焦点]
    C --> D[快捷键或明确目标控件]
    D --> E[React 导航与表单]
    E --> F[原有 Tauri / Rust core 操作]
    F --> G[忙碌、成功、失败反馈]
    G --> A
```

应用能改善的是 C、D、E、G。模型请求、工具通信和系统截图本身的延迟不在 MUX 控制范围内。

## 源码问题与改动

| 发现 | 原因及影响 | 实现 |
| --- | --- | --- |
| 搜索框打开后失焦 | `SearchBar` 的原生 `autoFocus` 之后，Modal 下一帧又聚焦标题；需要额外点击搜索框 | `desktop/src/components/ui.tsx:173` 为自动聚焦搜索框声明初始焦点；Modal 统一选择可见焦点目标 |
| 叠加弹窗留下可交互下层 | 之前只对 `#root` 设置 inert，挂到 body 的下层 portal 没被隔离 | `desktop/src/components/ui.tsx:243` 登记已挂载窗口，按 DOM 层级隔离非顶层 overlay；关闭后恢复原 inert 状态 |
| 关闭后焦点恢复不可靠 | 在子控件自动聚焦之后才记录 opener，可能记录到即将卸载的输入框 | `desktop/src/components/ui.tsx:347` 在渲染时保存入口，卸载后仅向仍连接且属于当前层的入口恢复焦点 |
| Agent 需要手牌翻页定位 | 没有全局直接入口，选择还需等待装饰过渡 | `desktop/src/components/AgentNavigation.tsx:26` 增加 Cmd/Ctrl+K；`desktop/src/components/AgentHandPicker.tsx:342` 提供立即进入路径 |
| 资源卡片多、动作重名 | 操作目标需要依赖位置和临近文字判断 | `desktop/src/components/AgentConsumptionPanel.tsx:38` 给收录、恢复等动作加资源名称及来源/目标；`:244` 增加当前资源筛选 |
| 中央 MCP 同名项难区分 | 同名的不同来源共用相同可访问名称 | `desktop/src/components/RegistryView.tsx:506` 名称补充 transport 与来源，不包含 URL 或凭据 |
| 候选列表缺少方向键约定 | 用户/Agent 需要逐个 Tab 或鼠标定位 | `desktop/src/components/ui.tsx:321` 共用候选导航；两个 picker 使用简短名称与单独描述 |
| 通知易错过 | 成功/错误均很快消失且缺乏状态语义 | `desktop/src/components/Toast.tsx:20` 成功为 status，6 秒消失；失败为 alert，保留到明确关闭；清理卸载计时器 |

### Agent 快速入口

`AgentNavigation` 只在没有其他弹窗时打开手牌。已打开手牌时重复快捷键会聚焦并选中搜索文本；正在编辑其他弹窗时不会切走。

搜索 Enter 的解析规则来自 `desktop/src/components/AgentHandPicker.tsx:451`：

1. 优先精确匹配 Agent ID。
2. 其次匹配唯一的完整名称。
3. 没有以上匹配时，只有筛选结果唯一才允许进入。
4. 多个候选时停留并提示，输入法组合中的 Enter 不执行导航。

键盘或可访问性激活卡片时使用同一个 `select` 入口，但立即导航；真实鼠标点击保留手牌入场动画。快速路径仍受保存、忙碌和替换状态的原有锁控制。它只选择 Agent，不提交资源配置。

### 弹窗层与焦点

`Modal` 使用已挂载集合与 inert 引用计数，避免 React 挂载/卸载阶段的短暂 DOM 状态误判顶层。下层 overlay 被隔离后，不参与点击、Tab 和可访问性目标选择。只有顶层处理 Escape/Tab；关闭子层后先解除父层隔离，再恢复触发控件焦点。

这遵循 W3C 的 [Modal Dialog Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/)：对话框打开时移动焦点、约束内部键盘导航，并让背景不可交互。业务上的忙碌状态由 `desktop/src/components/DialogShell.tsx:21` 继续阻止关闭，同时传递给 dialog 的 `aria-busy`。

### 资源查找与选择

`desktop/src/components/Layout.tsx:81` 的 Cmd/Ctrl+F 优先查找顶层弹窗内的搜索框，否则聚焦当前工作区搜索框；不会越过弹窗去操作底层搜索。当前 Agent 资源搜索覆盖名称、描述与资产标识；它只改变显示结果，不改资源状态。

资源 picker 的 Down/Up 从搜索框进入可用候选；候选之间支持方向键及 Home/End。聚焦与选中分离，Enter/Space 选择后仍须点击原有“添加”等提交按钮。搜索框的 Home/End 保留文本编辑用途。禁用候选不能被快捷键选中。

候选名称与描述分离，避免长描述成为控件主名称。该设计参考 [W3C Listbox Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/listbox/) 对清晰选项名称、焦点和选择状态的区分。

筛选不会改变“全部启用/停用/移除”的范围；这些按钮仍作用于当前能力下的完整列表，并走原有处理及必要确认。

## 给使用者和 Agent 的操作约定

| 目标 | 路径 | 完成依据 |
| --- | --- | --- |
| 切换 Agent | Cmd/Ctrl+K → 输入明确 ID 或完整名称 → Enter | 主工作区名称变为目标 Agent，选择器关闭 |
| 查找当前资源 | Cmd/Ctrl+F → 输入资源名称或描述 | 筛选数量及卡片列表更新 |
| 选择可用资源 | 打开添加窗口 → 输入搜索 → Down/Up → Enter/Space | 选中状态变化，提交按钮可用；尚未写入 |
| 提交资源操作 | 明确点击提交，按原有流程处理风险确认 | 忙碌结束后的结果或错误反馈 |
| 退出弹窗 | Escape 或关闭/取消 | 下层或入口焦点恢复；忙碌时不允许退出 |

可访问性工具返回的数字索引不是持久 ID。页面、搜索结果或弹窗变化后应重新观察并按角色、名称和状态定位，不能复用之前的控件编号。屏幕坐标操作同样应基于当前画面。

## 架构取舍与边界

- 共用 `Modal`、`SearchBar` 和候选导航，避免在每个资产页面复制一套自动化逻辑。没有新增运行依赖、监听端口或远程执行权限。
- 保留物理鼠标动画，只给明确的键盘/语义激活提供直接导航，兼顾原有视觉体验和操作效率。
- 新名称只使用已有公开展示元数据，不额外展开配置内容或凭据。
- `aria-busy`、status 和 alert 提供可观察状态；不能保证每一家工具都会输出这些属性。Toast 仍在应用根层，弹窗期间根层 inert，弹窗自身的错误/忙碌反馈仍由 DialogShell 展示。
- 本次没有修改 core 的备份、CAS、风险确认或提交语义。导航快捷键不能跨越正在编辑的弹窗，也不能把候选聚焦等同于写入授权。

## 验证范围

新增 `desktop/src/components/ComputerUseAccessibility.test.tsx`，覆盖弹窗搜索焦点、下层隔离与恢复、禁用候选跳过、候选导航不自动提交、同名 Agent 歧义、输入法 Enter、编辑弹窗保护以及错误通知保留；原资源动作名称的用例同步调整，并补充资源筛选场景。

按当前仓库极速交付规则，本次不执行自动化测试套件。交付经过生产编译、正式 Release 资产复验，并在安装版上验收实际快捷键与可访问性行为；对应运行结果在本次交付说明中记录。未把静态截图作为帧率或端到端耗时测试证据。
