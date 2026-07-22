import { expect, it } from "vitest";
import { resourceInitial } from "./resourceInitial";

it("uses the first visible letter or number as a resource avatar", () => {
  expect(resourceInitial("ali-employee-assistant")).toBe("A");
  expect(resourceInitial("  android-mcp")).toBe("A");
  expect(resourceInitial("@context7")).toBe("C");
  expect(resourceInitial("42-tools")).toBe("4");
  expect(resourceInitial("---", "M")).toBe("M");
});
