# Agent 拖拽实时预览设计

## 目标

置顶 Agent 排序在拖动期间直接展示最终顺序，而不是只降低原行透明度、等松手后突然换位。下拉列表与顶部置顶图标栏必须同步预览；只有完成 drop 才写入设置。

## 交互

- 仍从已置顶行左侧的拖拽手柄开始操作。
- 拖过目标行上半区时预览插入到目标前方，拖过下半区时预览插入到目标后方。
- 每次有效目标变化后，已置顶列表立即按预览顺序让位；顶部图标栏同步显示相同顺序。
- 被拖行保持可识别，使用降低透明度、轻微缩放和抓取光标表达“正在移动”，不额外增加说明文案。
- drop 时提交当前预览顺序一次。保存成功后保留该顺序并播报新位置；保存失败时沿用现有 toast，并由 `usePinnedAgents` 回滚到已保存顺序。
- dragend、Escape、关闭选择器或拖出有效区域后未 drop，均清除预览并恢复原顺序。
- `Option + Up/Down` 键盘排序保持现状，不与拖拽预览状态并行执行。

## 状态与数据流

`AgentNavigation` 增加仅存在于一次拖拽会话的预览状态：

- `draggedId`：当前拖动的 Agent。
- `previewIds`：基于拖拽开始时已保存顺序计算的本地预览。
- `dropTarget`：当前目标 Agent 与 before/after 位置，用于避免同一位置的重复 state update，并提供视觉状态。

渲染顺序在拖拽中取 `previewIds`，其余时间取 `agentIds`。Agent 信息仍来自同一 `agents` 映射，顶部图标栏与下拉列表共用这份渲染顺序，不创建第二套排序状态。

`dragover` 只调用纯函数计算下一预览顺序；相同顺序不更新 React state。`drop` 将当前 `previewIds` 交给现有 `commit`，其乐观更新、串行保存和失败回滚逻辑不变。拖拽过程中不调用 Tauri command、不写 `settings.json`。

## 实现边界

- 延续现有 HTML5 drag events 和 `dragDropEnabled: false` 的 Tauri 窗口契约，不引入拖拽依赖。
- 排序纯函数继续放在 `desktop/src/lib/pinnedAgents.ts`；组件只管理会话状态和事件。
- CSS 只增加拖动行、潜在落点和顺序变化的短过渡。`prefers-reduced-motion: reduce` 下关闭位移动画，但保留顺序与状态反馈。
- 最多六个置顶 Agent、Pin/取消 Pin、搜索、Agent 选择和保存并发限制均不改变。

## 验证

- 纯函数测试覆盖连续跨多行、before/after 边界、自身目标、未知 ID 和输入不可变。
- 组件/CSS 静态门禁覆盖：dragover 更新预览、顶部与列表共用预览顺序、drop 才调用 commit、拖动/目标视觉状态及 reduced-motion。
- 执行桌面单测、图标检查、生产构建、Rust workspace/Tauri 测试和网站构建。
- 正式安装版验收：拖动时列表与顶部图标同步换位；取消恢复；drop 后重启仍保持顺序；`900x600` 与 `1200x820` 下选择器不溢出。

## 非目标

- 不允许直接拖动顶部图标排序。
- 不重写为 pointer-based 拖拽系统，也不新增 dnd-kit。
- 不改变置顶数量、持久化格式或 Agent 选择器信息架构。
