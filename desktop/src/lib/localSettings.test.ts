import { afterEach, expect, it } from "vitest";
import { readLocalSetting, writeLocalSetting } from "./localSettings";

const descriptor = Object.getOwnPropertyDescriptor(window, "localStorage")!;
afterEach(() => Object.defineProperty(window, "localStorage", descriptor));

it("keeps optional preferences usable when browser storage access is blocked", () => {
  Object.defineProperty(window, "localStorage", { configurable: true,
    get() { throw new DOMException("blocked", "SecurityError"); } });
  expect(readLocalSetting("blocked-fixture")).toBeNull();
  writeLocalSetting("blocked-fixture", "280");
  expect(readLocalSetting("blocked-fixture")).toBe("280");
  writeLocalSetting("blocked-fixture", null);
  expect(readLocalSetting("blocked-fixture")).toBeNull();
});

it("retains a new preference for this session when a quota failure prevents persistence", () => {
  Object.defineProperty(window, "localStorage", { configurable: true, value: {
    getItem: () => "old", setItem: () => { throw new DOMException("full", "QuotaExceededError"); },
  } });
  expect(readLocalSetting("quota-fixture")).toBe("old");
  writeLocalSetting("quota-fixture", "new");
  expect(readLocalSetting("quota-fixture")).toBe("new");
});
