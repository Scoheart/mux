import { defineConfig } from "vitepress";

// https://vitepress.dev/reference/site-config
export default defineConfig({
  lang: "zh-CN",
  title: "MUX",
  description: "跨 AI 编码 agent 统一管理 MCP 服务器 · 官方文档",
  cleanUrls: true,
  lastUpdated: true,

  head: [
    ["meta", { name: "theme-color", content: "#FF7A59" }],
    ["meta", { property: "og:title", content: "MUX — MCP Multiplexer" }],
    ["meta", { property: "og:description", content: "一处管理你所有 AI 编码 agent 的 MCP 服务器" }],
  ],

  themeConfig: {
    nav: [
      { text: "指南", link: "/guide/what-is-mux", activeMatch: "/guide/" },
      { text: "支持的 Agent", link: "/guide/agents" },
      { text: "下载", link: "https://github.com/Scoheart/mux/releases" },
    ],

    sidebar: {
      "/guide/": [
        {
          text: "开始",
          items: [
            { text: "MUX 是什么", link: "/guide/what-is-mux" },
            { text: "安装", link: "/guide/install" },
            { text: "核心概念", link: "/guide/concepts" },
          ],
        },
        {
          text: "使用",
          items: [
            { text: "桌面 App 指南", link: "/guide/desktop" },
            { text: "命令行 / TUI", link: "/guide/cli" },
            { text: "支持的 Agent", link: "/guide/agents" },
          ],
        },
        {
          text: "参考",
          items: [
            { text: "常见问题", link: "/guide/faq" },
          ],
        },
      ],
    },

    socialLinks: [{ icon: "github", link: "https://github.com/Scoheart/mux" }],

    editLink: {
      pattern: "https://github.com/Scoheart/mux/edit/main/website/:path",
      text: "在 GitHub 上编辑此页",
    },

    docFooter: { prev: "上一页", next: "下一页" },
    outline: { label: "本页导航", level: [2, 3] },
    lastUpdatedText: "最后更新",
    returnToTopLabel: "回到顶部",
    darkModeSwitchLabel: "外观",
    sidebarMenuLabel: "菜单",

    footer: {
      message: "MIT Licensed",
      copyright: "© 2026 Scoheart · MUX",
    },

    search: { provider: "local" },
  },
});
