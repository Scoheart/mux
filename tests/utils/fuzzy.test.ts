import { describe, it, expect } from "vitest";
import { createMcpSearcher } from "../../src/utils/fuzzy.js";
import type { RegistryEntry } from "../../src/types.js";

const entries: RegistryEntry[] = [
  { name: "chrome-devtools", description: "Chrome CDP debugging", tags: ["debug", "browser"], config: {} as any },
  { name: "filesystem", description: "文件系统访问", tags: ["fs", "files"], config: {} as any },
  { name: "github", description: "GitHub API", tags: ["git", "api"], config: {} as any },
];

describe("createMcpSearcher", () => {
  it("finds by name substring", () => {
    const searcher = createMcpSearcher(entries);
    const results = searcher("chrome");
    expect(results[0].name).toBe("chrome-devtools");
  });

  it("finds by tag", () => {
    const searcher = createMcpSearcher(entries);
    const results = searcher("files");
    expect(results[0].name).toBe("filesystem");
  });

  it("finds by description", () => {
    const searcher = createMcpSearcher(entries);
    const results = searcher("GitHub");
    expect(results[0].name).toBe("github");
  });

  it("returns all entries for empty query", () => {
    const searcher = createMcpSearcher(entries);
    const results = searcher("");
    expect(results).toHaveLength(3);
  });
});
