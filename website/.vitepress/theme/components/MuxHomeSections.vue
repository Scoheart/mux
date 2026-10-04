<script setup lang="ts">
import { computed } from "vue";
import { withBase } from "vitepress";
import reference from "../../reference.json";
import MuxDemo from "./MuxDemo.vue";
const props = defineProps<{ lang?: "zh" | "en" }>();
const en = computed(() => props.lang === "en");
const prefix = computed(() => en.value ? '/en/guide/' : '/guide/');
const rows = computed(() => en.value ? [
  { title: 'One connection. Several Agents.', desc: 'Create a Provider once, add Models, and choose which compatible Agents use them. Account portals and protocol paths stay visible and editable.', href: 'models', cta: 'Models & Providers', image: 'provider' },
  { title: 'A Skill library with clear sources.', desc: 'Download or import one central copy. Browse by source, read three-line descriptions, and open consumers through their Agent icons.', href: 'skills', cta: 'Manage Skills', image: 'skills' },
  { title: 'Tools in their native configuration.', desc: 'Add, paste, import, or subscribe to MCPs. Assign from the central library while preserving each Agent’s unrelated settings.', href: 'desktop', cta: 'Desktop guide', image: 'mcps' },
] : [
  { title: '一份连接，交给多个 Agent。', desc: '创建 Provider、添加模型，再选择兼容 Agent。服务商入口、模型列表与协议路径直接展示，随时可以编辑。', href: 'models', cta: 'Models 与 Providers', image: 'provider' },
  { title: '每个 Skill，都能找到来源。', desc: '下载或导入一份中央副本。左侧按来源浏览，卡片展示三行简介，Agent 图标直接连接到使用它的客户端。', href: 'skills', cta: '管理 Skills', image: 'skills' },
  { title: '让工具进入原生配置。', desc: '添加、粘贴、导入或订阅 MCPs，再从中央库分配给 Agent。原生格式由 MUX 适配，无关设置继续保留。', href: 'desktop', cta: '桌面 App 指南', image: 'mcps' },
]);
</script>

<template>
  <div class="mux-home">
    <section id="product-demo" class="mux-product-demo" aria-labelledby="demo-title">
      <div class="mux-product-demo__meta"><span>v{{ reference.version }} · macOS Apple Silicon</span><a :href="withBase(prefix + 'agents')">{{ reference.agents.length }} {{ en ? 'audited Agents' : '个核验 Agent' }} →</a></div>
      <h2 id="demo-title">{{ en ? 'See the workspace in action.' : '看看它如何工作。' }}</h2>
      <p>{{ en ? 'Models, MCPs, Skills, and an Agent picker — in one desktop workspace.' : 'Models、MCPs、Skills 与 Agent 选择器，都在同一个桌面工作区。' }}</p>
      <MuxDemo :lang="lang" />
    </section>
    <section class="mux-story">
      <header class="mux-story__header" data-reveal><h2 class="mux-story__heading">{{ en ? 'Configure once. Choose where it goes.' : '配置一次，决定交给谁。' }}</h2></header>
      <article v-for="(row, i) in rows" :key="row.href" class="mux-row" :class="{ 'is-reverse': i % 2 === 1 }" data-reveal>
        <div class="mux-row__visual mux-row__visual--screenshot"><img :src="withBase('/media/mux-1.10.0-' + row.image + '.jpg')" :alt="row.title" width="1440" height="984" loading="lazy" /></div>
        <div class="mux-row__text"><h3 class="mux-row__title">{{ row.title }}</h3><p class="mux-row__desc">{{ row.desc }}</p><a class="mux-try" :href="withBase(prefix + row.href)">{{ row.cta }} <span aria-hidden="true">→</span></a></div>
      </article>
    </section>
    <section class="mux-automation" data-reveal>
      <div><span class="mux-eyebrow">DESKTOP + CLI</span><h2>{{ en ? 'Give your Agent the same controls.' : '也让你的 Agent 操作这份资源库。' }}</h2><p>{{ en ? 'Batch status reads, inspect local traces, and review a plan before committing it in the original process. Desktop and CLI share the same assets and safety rules.' : '批量查询状态、读取本地 Trace，把计划交给人或其他 Agent 审阅，再执行原计划。桌面与 CLI 共用资产和写入规则。' }}</p><a class="mux-try" :href="withBase(prefix + 'cli')">{{ en ? 'CLI reference' : 'CLI 使用指南' }} →</a></div>
      <pre><code>mux status --agent codex --agent opencode --json
mux trace list --agent codex --json
mux operation review --file request.json --json</code></pre>
    </section>
    <section class="mux-cta" data-reveal><h2 class="mux-cta__title">{{ en ? 'Start with the Agents you already use.' : '从你正在使用的 Agent 开始。' }}</h2><p class="mux-cta__desc">{{ en ? 'Install MUX, create a central asset, then choose its consumers.' : '安装 MUX，创建一份中央资产，再选择需要使用它的客户端。' }}</p><div class="mux-cta__actions"><a class="mux-btn mux-btn--brand" :href="withBase(prefix + 'install')">{{ en ? 'Install MUX' : '安装 MUX' }}</a><a class="mux-btn mux-btn--alt" :href="withBase(prefix + 'providers')">{{ en ? 'Provider templates' : 'Provider 模板' }}</a></div></section>
  </div>
</template>
