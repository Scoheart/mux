import { nextTick, watch, onMounted } from "vue";
import type { EnhanceAppContext, Router } from "vitepress";
import DefaultTheme from "vitepress/theme";
import { useRoute } from "vitepress";
import Layout from "./Layout.vue";
import MuxHomeSections from "./components/MuxHomeSections.vue";
import { initScrollReveal } from "./scroll-reveal";
import "./custom.css";

function prefersReducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function canViewTransition(): boolean {
  return "startViewTransition" in document && !prefersReducedMotion();
}

/** Wrap VitePress SPA navigations in a View Transition (fade + slight rise). */
function installRouteTransitions(router: Router): void {
  if (typeof window === "undefined") return;

  const go = router.go.bind(router);
  router.go = async (to) => {
    if (!canViewTransition()) {
      await go(to);
      return;
    }

    const transition = document.startViewTransition(async () => {
      await go(to);
      await nextTick();
    });

    try {
      await transition.finished;
    } catch {
      // Navigation superseded / interrupted.
    }
  };
}

export default {
  extends: DefaultTheme,
  Layout,
  enhanceApp({ app, router }: EnhanceAppContext) {
    app.component("MuxHomeSections", MuxHomeSections);
    installRouteTransitions(router);
  },
  setup() {
    const route = useRoute();

    const refreshReveal = () => {
      void nextTick(() => initScrollReveal());
    };

    onMounted(refreshReveal);
    watch(() => route.path, refreshReveal);
  },
};
