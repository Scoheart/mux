import { describe, it, expect } from "vitest";
import { expandTilde, resolvePath } from "../../src/utils/path.js";
import { homedir } from "node:os";

describe("expandTilde", () => {
  it("expands ~ to home directory", () => {
    expect(expandTilde("~/.claude.json")).toBe(`${homedir()}/.claude.json`);
  });

  it("leaves absolute paths unchanged", () => {
    expect(expandTilde("/usr/local/bin")).toBe("/usr/local/bin");
  });

  it("leaves relative paths unchanged", () => {
    expect(expandTilde(".cursor/mcp.json")).toBe(".cursor/mcp.json");
  });
});

describe("resolvePath", () => {
  it("resolves global path with tilde expansion", () => {
    const result = resolvePath("~/.claude.json", "global");
    expect(result).toBe(`${homedir()}/.claude.json`);
  });

  it("resolves project path relative to projectDir", () => {
    const result = resolvePath(".mcp.json", "project", "/Users/test/myproject");
    expect(result).toBe("/Users/test/myproject/.mcp.json");
  });

  it("throws if project scope but no projectDir", () => {
    expect(() => resolvePath(".mcp.json", "project")).toThrow();
  });
});
