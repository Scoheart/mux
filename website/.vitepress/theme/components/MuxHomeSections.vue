<script setup lang="ts">
import { computed, onMounted, nextTick } from "vue";
import { useData, withBase } from "vitepress";
import { initScrollReveal } from "../scroll-reveal";

const props = defineProps<{
  /** Site locale: zh (default) or en */
  lang?: "zh" | "en";
}>();

const { isDark } = useData();
const isEn = computed(() => props.lang === "en");

const shotSrc = computed(() =>
  withBase(
    isDark.value
      ? "/img/registry-overview-dark.png"
      : "/img/registry-overview-light.png",
  ),
);

const shotAlt = computed(() =>
  isEn.value
    ? isDark.value
      ? "MUX desktop Registry in dark mode"
      : "MUX desktop Registry in light mode"
    : isDark.value
      ? "MUX 桌面 App 暗色模式 Registry"
      : "MUX 桌面 App 浅色模式 Registry",
);

const copy = computed(() =>
  isEn.value
    ? {
        shotTitle: "One catalog for MCP across every AI coding Agent",
        shotDesc:
          "Browse the Registry and install MCP into Claude Code, Cursor, Codex, and more — MUX writes each Agent’s native config format and path.",
        sectionHeading: "Built for how you actually work",
        rows: [
          {
            title: "Source-driven catalog",
            desc: "Subscribe to remote URLs, import local files, paste configs, or let MUX discover what’s already in your agents. No hardcoded server list.",
            cta: "Core concepts",
            href: "/en/guide/concepts",
            visual: "sources",
          },
          {
            title: "Desktop + CLI, one data dir",
            desc: "The macOS app and the native Rust CLI / TUI share ~/.mux. Change it once — both sides stay in sync.",
            cta: "CLI / TUI",
            href: "/en/guide/cli",
            visual: "dual",
          },
          {
            title: "Safe, local writes",
            desc: "MUX edits only the target MCP entry. Backups first, atomic replace, comments and policy fields preserved. Nothing leaves your machine.",
            cta: "FAQ",
            href: "/en/guide/faq",
            visual: "safe",
          },
        ],
        ctaTitle: "Start in under a minute",
        ctaDesc: "Install the desktop app or the CLI, then manage MCP across your agents from one place.",
        ctaPrimary: "Install",
        ctaPrimaryHref: "/en/guide/install",
        ctaSecondary: "What is MUX",
        ctaSecondaryHref: "/en/guide/what-is-mux",
      }
    : {
        shotTitle: "一份目录，管理所有 AI 编码 Agent 的 MCP",
        shotDesc:
          "在 Registry 中浏览 MCP，一键安装到 Claude Code、Cursor、Codex 等 Agent；配置格式与路径由 MUX 自动写入。",
        sectionHeading: "按真实工作流设计",
        rows: [
          {
            title: "来源驱动，不写死清单",
            desc: "订阅远程 URL、导入本地文件、粘贴配置，或自动探索各 agent 已有 MCP。目录随来源刷新更新。",
            cta: "核心概念",
            href: "/guide/concepts",
            visual: "sources",
          },
          {
            title: "桌面 + 命令行，同一份数据",
            desc: "macOS 桌面 App 与原生 Rust CLI / TUI 共享 ~/.mux。一处改动，两端同步。",
            cta: "命令行 / TUI",
            href: "/guide/cli",
            visual: "dual",
          },
          {
            title: "安全、本地写入",
            desc: "只改目标 MCP 条目：先备份、再原子替换，保留注释与策略字段。完整配置不会上传。",
            cta: "常见问题",
            href: "/guide/faq",
            visual: "safe",
          },
        ],
        ctaTitle: "一分钟开始",
        ctaDesc: "安装桌面 App 或 CLI，从一处管理所有 AI 编码 agent 的 MCP。",
        ctaPrimary: "安装",
        ctaPrimaryHref: "/guide/install",
        ctaSecondary: "MUX 是什么",
        ctaSecondaryHref: "/guide/what-is-mux",
      },
);

onMounted(async () => {
  await nextTick();
  initScrollReveal();
});
</script>

<template>
  <div class="mux-home">
    <!-- Product screenshot band -->
    <section class="mux-shot" data-reveal>
      <div class="mux-shot__inner">
        <h2 class="mux-shot__title">{{ copy.shotTitle }}</h2>
        <p class="mux-shot__desc">{{ copy.shotDesc }}</p>
        <div class="mux-shot__frame">
          <img
            class="mux-shot__img"
            :src="shotSrc"
            :alt="shotAlt"
            width="2400"
            height="1642"
            loading="lazy"
          />
        </div>
      </div>
    </section>

    <!-- Alternating story rows (Buzzy seo-feature pattern) -->
    <section class="mux-story">
      <header class="mux-story__header" data-reveal>
        <h2 class="mux-story__heading">{{ copy.sectionHeading }}</h2>
      </header>

      <article
        v-for="(row, i) in copy.rows"
        :key="row.title"
        class="mux-row"
        :class="{ 'is-reverse': i % 2 === 1 }"
        data-reveal
      >
        <div class="mux-row__visual" :data-visual="row.visual">
          <div class="mux-panel">
            <div v-if="row.visual === 'sources'" class="mux-panel__sources">
              <span>remote</span>
              <span>local</span>
              <span>manual</span>
              <span>discovered</span>
            </div>
            <div v-else-if="row.visual === 'dual'" class="mux-panel__dual">
              <div class="mux-panel__chip">Desktop</div>
              <div class="mux-panel__chip mux-panel__chip--accent">~/.mux</div>
              <div class="mux-panel__chip">CLI / TUI</div>
            </div>
            <div v-else class="mux-panel__safe">
              <code>backup → write → atomic replace</code>
            </div>
          </div>
        </div>
        <div class="mux-row__text">
          <h3 class="mux-row__title">{{ row.title }}</h3>
          <p class="mux-row__desc">{{ row.desc }}</p>
          <a class="mux-try" :href="withBase(row.href)">
            {{ row.cta }}
            <span aria-hidden="true">→</span>
          </a>
        </div>
      </article>
    </section>

    <!-- Bottom CTA -->
    <section class="mux-cta" data-reveal>
      <h2 class="mux-cta__title">{{ copy.ctaTitle }}</h2>
      <p class="mux-cta__desc">{{ copy.ctaDesc }}</p>
      <div class="mux-cta__actions">
        <a class="mux-btn mux-btn--brand" :href="withBase(copy.ctaPrimaryHref)">
          {{ copy.ctaPrimary }}
        </a>
        <a class="mux-btn mux-btn--alt" :href="withBase(copy.ctaSecondaryHref)">
          {{ copy.ctaSecondary }}
        </a>
      </div>
    </section>
  </div>
</template>
