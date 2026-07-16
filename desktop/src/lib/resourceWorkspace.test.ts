import assert from "node:assert/strict";
import test from "node:test";
import {
  DEFAULT_SIDEBAR_WIDTH,
  clampSidebarWidth,
  parseSidebarWidth,
  redactSensitiveConfig,
} from "./resourceWorkspace.ts";

test("sidebar width is clamped and invalid storage falls back", () => {
  assert.equal(clampSidebarWidth(100), 184);
  assert.equal(clampSidebarWidth(260), 260);
  assert.equal(clampSidebarWidth(500), 340);
  assert.equal(parseSidebarWidth(null), DEFAULT_SIDEBAR_WIDTH);
  assert.equal(parseSidebarWidth("invalid"), DEFAULT_SIDEBAR_WIDTH);
  assert.equal(parseSidebarWidth("312"), 312);
});

test("sensitive fields are recursively redacted without mutating input", () => {
  const source = {
    env: {
      API_TOKEN: "token-value",
      ALIBABA_CLOUD_ACCESS_KEY_SECRET: "secret-value",
      REGION: "cn-shenzhen",
    },
    nested: [{ password: "password-value", enabled: true }],
  };
  const result = redactSensitiveConfig(source);
  assert.deepEqual(result, {
    env: {
      API_TOKEN: "••••••••",
      ALIBABA_CLOUD_ACCESS_KEY_SECRET: "••••••••",
      REGION: "cn-shenzhen",
    },
    nested: [{ password: "••••••••", enabled: true }],
  });
  assert.equal(source.env.API_TOKEN, "token-value");
});
