import { expect, it } from "vitest";
import {
  DEFAULT_SIDEBAR_WIDTH,
  clampSidebarWidth,
  parseSidebarWidth,
  redactSensitiveConfig,
} from "./resourceWorkspace.ts";

it("sidebar width is clamped and invalid storage falls back", () => {
  expect(clampSidebarWidth(100)).toBe(184);
  expect(clampSidebarWidth(260)).toBe(260);
  expect(clampSidebarWidth(500)).toBe(340);
  expect(parseSidebarWidth(null)).toBe(DEFAULT_SIDEBAR_WIDTH);
  expect(parseSidebarWidth("invalid")).toBe(DEFAULT_SIDEBAR_WIDTH);
  expect(parseSidebarWidth("183")).toBe(DEFAULT_SIDEBAR_WIDTH);
  expect(parseSidebarWidth("341")).toBe(DEFAULT_SIDEBAR_WIDTH);
  expect(parseSidebarWidth("312")).toBe(312);
});

it("sensitive fields are recursively redacted without mutating input", () => {
  const source = {
    env: {
      API_TOKEN: "token-value",
      ALIBABA_CLOUD_ACCESS_KEY_SECRET: "secret-value",
      REGION: "cn-shenzhen",
    },
    headers: {
      Authorization: "Bearer credential-value",
      Cookie: "session=credential-value",
      "Set-Cookie": "refresh=credential-value",
      "X-Request-Id": "request-id",
    },
    nested: [{ password: "password-value", "Proxy-Authorization": "Basic credential-value", enabled: true }],
  };
  const result = redactSensitiveConfig(source);
  expect(result).toEqual({
    env: {
      API_TOKEN: "••••••••",
      ALIBABA_CLOUD_ACCESS_KEY_SECRET: "••••••••",
      REGION: "cn-shenzhen",
    },
    headers: {
      Authorization: "••••••••",
      Cookie: "••••••••",
      "Set-Cookie": "••••••••",
      "X-Request-Id": "request-id",
    },
    nested: [{ password: "••••••••", "Proxy-Authorization": "••••••••", enabled: true }],
  });
  expect(source.env.API_TOKEN).toBe("token-value");
});
