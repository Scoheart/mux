import { defineConfig } from "vitepress";

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "MUX",
  cleanUrls: true,
  lastUpdated: true,

  head: [
    ["meta", { name: "theme-color", content: "#7C5CFC" }],
    ["meta", { property: "og:title", content: "MUX — MCP Multiplexer" }],
    [
      "meta",
      {
        property: "og:description",
        content: "One place to manage your MCP servers across every AI coding agent",
      },
    ],
  ],

  themeConfig: {
    socialLinks: [{ icon: "github", link: "https://github.com/Scoheart/mux" }],
    search: { provider: "local" },
    externalLinkIcon: true,
  },

  locales: {
    root: {
      label: "简体中文",
      lang: "zh-CN",
      description: "跨 AI 编码 agent 统一管理 MCP 服务器 · 官方文档",
      themeConfig: {
        nav: [
          { text: "首页", link: "/" },
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
              items: [{ text: "常见问题", link: "/guide/faq" }],
            },
          ],
        },
        editLink: {
          pattern: "https://github.com/Scoheart/mux/edit/main/website/:path",
          text: "在 GitHub 上编辑此页",
        },
        docFooter: { prev: "上一页", next: "下一页" },
        outline: { label: "本页导航", level: [2, 3] },
        lastUpdatedText: "最后更新",
        returnToTopLabel: "回到顶部",
        darkModeSwitchLabel: "外观",
        lightModeSwitchTitle: "切换到浅色模式",
        darkModeSwitchTitle: "切换到深色模式",
        sidebarMenuLabel: "菜单",
        langMenuLabel: "切换语言",
        footer: {
          message: "MIT Licensed",
          copyright: "© 2026 Scoheart · MUX",
        },
      },
    },

    en: {
      label: "English",
      lang: "en-US",
      link: "/en/",
      description: "Manage MCP servers across every AI coding agent · Official docs",
      themeConfig: {
        nav: [
          { text: "Home", link: "/en/" },
          { text: "Guide", link: "/en/guide/what-is-mux", activeMatch: "/en/guide/" },
          { text: "Agents", link: "/en/guide/agents" },
          { text: "Download", link: "https://github.com/Scoheart/mux/releases" },
        ],
        sidebar: {
          "/en/guide/": [
            {
              text: "Getting started",
              items: [
                { text: "What is MUX", link: "/en/guide/what-is-mux" },
                { text: "Installation", link: "/en/guide/install" },
                { text: "Core concepts", link: "/en/guide/concepts" },
              ],
            },
            {
              text: "Usage",
              items: [
                { text: "Desktop app", link: "/en/guide/desktop" },
                { text: "CLI / TUI", link: "/en/guide/cli" },
                { text: "Supported agents", link: "/en/guide/agents" },
              ],
            },
            {
              text: "Reference",
              items: [{ text: "FAQ", link: "/en/guide/faq" }],
            },
          ],
        },
        editLink: {
          pattern: "https://github.com/Scoheart/mux/edit/main/website/:path",
          text: "Edit this page on GitHub",
        },
        footer: {
          message: "MIT Licensed",
          copyright: "© 2026 Scoheart · MUX",
        },
      },
    },
  },
});
