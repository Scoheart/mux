# Provider 上下文新增 Model 锁定设计

## 背景与目标

Models 工作区同时支持“全部模型”和具体 Provider 两种上下文。用户已经进入具体 Provider 后点击“添加模型”，当前弹窗仍显示 Provider 下拉框，重复暴露了不属于当前上下文的选择。

本改动让新增入口遵循当前工作区上下文：

| 入口 | Provider 字段 | 行为 |
|---|---|---|
| 全部模型 | 可切换下拉框 | 默认首个 Provider，用户可改选 |
| 具体 Provider | 只读字段 | 固定为当前 Provider，不提供切换能力 |
| 编辑已有 Model | 可切换下拉框 | 保留现有迁移到其他 Provider 的能力 |

## 交互与状态

- 具体 Provider 内打开新增弹窗时，仍展示 Provider 名称，帮助用户确认归属，但用只读文本字段替代下拉框和展开箭头。
- 新增弹窗继续自动使用该 Provider 的协议、Base URL、凭据和模型发现能力。
- Provider 不支持模型发现或发现失败时，Model ID 仍可手工填写。
- “全部模型”入口与编辑已有 Model 的 Provider 选择不变。

## 实现方案

`ModelsView` 根据新增动作发生时的 `providerFilter` 判断是否来自具体 Provider，并向 `ModelProfileDialog` 传递显式的 `providerSelectionLocked`。弹窗只在该标志为真且 Provider 仍存在时渲染只读字段，否则继续使用原有 `FormSelect`。

锁定仅是界面上下文约束，不改变 Model/Provider schema、Core 写入契约或模型发现 API。保存时仍提交同一个持久化 Provider instance ID。

## 验收

- 进入具体 Provider 后新增 Model，看得到当前 Provider 名称但没有 Provider combobox。
- 新增 Model 仍自动发现该 Provider 的可用模型并可手填 Model ID。
- 从全部模型新增时仍可切换 Provider。
- 编辑已有 Model 时仍可切换 Provider。

