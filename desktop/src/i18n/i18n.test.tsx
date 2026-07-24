import fs from "node:fs";
import path from "node:path";
import { render, waitFor } from "@testing-library/react";
import { useTranslation } from "react-i18next";
import { describe, expect, it } from "vitest";
import { legacyEnglish, localizeLegacyText } from "./legacy";
import i18n, { translationResources } from ".";
import { LegacyLocalizationBridge } from "./LegacyLocalizationBridge";

function leafKeys(value: unknown, prefix = ""): string[] {
  if (!value || typeof value !== "object") return [prefix];
  return Object.entries(value).flatMap(([key, child]) =>
    leafKeys(child, prefix ? `${prefix}.${key}` : key)
  );
}

describe("Desktop i18n contract", () => {
  it("keeps zh-CN and en-US typed dictionaries structurally identical", () => {
    expect(leafKeys(translationResources["en-US"]).sort())
      .toEqual(leafKeys(translationResources["zh-CN"]).sort());
  });

  it("translates legacy product copy without touching unknown dynamic content", () => {
    expect(localizeLegacyText("取消", "en-US")).toBe("Cancel");
    expect(localizeLegacyText("  读取失败  ", "en-US")).toBe("  Could not load  ");
    expect(localizeLegacyText("Claude Code", "en-US")).toBe("Claude Code");
    expect(localizeLegacyText("取消", "zh-CN")).toBe("取消");
  });

  it("keeps every catalog key non-empty", () => {
    for (const [source, translated] of Object.entries(legacyEnglish)) {
      expect(source.trim(), source).not.toBe("");
      expect(translated.trim(), source).not.toBe("");
      expect(translated, source).not.toBe(source);
    }
  });

  it("switches legacy text and accessibility attributes in place and restores Chinese", async () => {
    const view = render(
      <>
        <div id="root"><button title="关闭">取消</button></div>
        <LegacyLocalizationBridge locale="en-US" />
      </>,
    );
    await waitFor(() => expect(view.getByRole("button", { name: "Cancel" })).toHaveAttribute("title", "Close"));
    view.rerender(
      <>
        <div id="root"><button title="关闭">取消</button></div>
        <LegacyLocalizationBridge locale="zh-CN" />
      </>,
    );
    await waitFor(() => expect(view.getByRole("button", { name: "取消" })).toHaveAttribute("title", "关闭"));
  });

  it("renders typed Desktop copy in English and can switch back to Chinese", async () => {
    function ModelProviderLabel() {
      const { t } = useTranslation();
      return <span>{t("models.provider")}</span>;
    }

    await i18n.changeLanguage("en-US");
    const view = render(<ModelProviderLabel />);
    expect(view.getByText("Model provider")).toBeVisible();

    await i18n.changeLanguage("zh-CN");
    await waitFor(() => expect(view.getByText("模型提供商")).toBeVisible());
  });

  it("covers static Chinese product copy remaining outside typed i18next surfaces", () => {
    const sourceRoot = path.resolve("src");
    const uncovered = new Set<string>();
    const walk = (directory: string) => {
      for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
        const file = path.join(directory, entry.name);
        if (entry.isDirectory()) {
          if (!["i18n", "test"].includes(entry.name)) walk(file);
          continue;
        }
        if (!/\.tsx?$/.test(entry.name) || /\.test\.tsx?$/.test(entry.name)) continue;
        const text = fs.readFileSync(file, "utf8");
        for (const line of text.split("\n")) {
          const trimmed = line.trim();
          if (
            !/[\u3400-\u9fff]/.test(line)
            || trimmed.startsWith("//")
            || trimmed.startsWith("*")
            || trimmed.startsWith("/*")
          ) continue;
          for (const match of line.matchAll(/(["'`])([^"'`]*[\u3400-\u9fff][^"'`]*)\1/g)) {
            const phrase = match[2].replace(/\s+/g, " ").trim();
            if (phrase && localizeLegacyText(phrase, "en-US") === phrase) uncovered.add(phrase);
          }
          for (const match of line.matchAll(/>([^<>{}]*[\u3400-\u9fff][^<>{}]*)</g)) {
            const phrase = match[1].replace(/\s+/g, " ").trim();
            if (phrase && localizeLegacyText(phrase, "en-US") === phrase) uncovered.add(phrase);
          }
        }
      }
    };
    walk(sourceRoot);
    expect([...uncovered].sort()).toEqual([]);
  });
});
