import assert from "node:assert/strict";
import test from "node:test";

import {
  assertNoCredentialLikeText,
  redact,
  safeFetchError,
  serializeSnapshot,
  snippets,
} from "./extract-agent-repository-evidence.mjs";

const demoSecret = "DEMO-NOT-A-REAL-SECRET-123456";

test("redacts typed sensitive assignments without leaving the real RHS", () => {
  const input = [
    `api_key: str = "${demoSecret}"`,
    `access_token: str | None = "${demoSecret}"`,
    "if self.api_key:",
    "    return self.api_key",
  ].join("\n");

  const output = redact(input);

  assert.equal(output.includes(demoSecret), false);
  assert.match(output, /^api_key: \[redacted-example\]$/m);
  assert.match(output, /^access_token: \[redacted-example\]$/m);
  assert.match(output, /^if self\.api_key: \[redacted-example\]$/m);
  assert.match(output, /^    return self\.api_key$/m);
});

test("redacts JSON, YAML, and assignment credential fields line by line", () => {
  const input = [
    `{"apiKey": "${demoSecret}", "safe": true}`,
    `password: ${demoSecret}`,
    `client_secret = "${demoSecret}"`,
    `openRouterApiKey: "${demoSecret}"`,
    'api_key_env: "SAFE_ENV_NAME"',
  ].join("\n");

  const output = redact(input);

  assert.equal(output.includes(demoSecret), false);
  assert.match(output, /"apiKey": \[redacted-example\]/);
  assert.match(output, /^password: \[redacted-example\]$/m);
  assert.match(output, /^client_secret = \[redacted-example\]$/m);
  assert.match(output, /^openRouterApiKey: \[redacted-example\]$/m);
  assert.match(output, /^api_key_env: "SAFE_ENV_NAME"$/m);
});

test("redacts Bearer values, URL userinfo, and common standalone key shapes", () => {
  const bearer = ["Bearer", "DEMO-BEARER-TOKEN-123456"].join(" ");
  const userinfoUrl = `https://${["demo-user", "demo-password"].join(":")}@example.com/api`;
  const tokenPlanKey = ["tp", "DEMO_TOKEN_PLAN_KEY_123456"].join("-");
  const huggingFaceToken = ["hf", "DEMOHUGGINGFACETOKEN1234567890"].join("_");
  const groqKey = ["gsk", "DEMOGROQKEY12345678901234567890"].join("_");
  const xaiKey = ["xai", "DEMOXAIKEY12345678901234567890"].join("-");
  const input = [
    `Authorization: ${bearer}`,
    userinfoUrl,
    tokenPlanKey,
    huggingFaceToken,
    groqKey,
    xaiKey,
  ].join("\n");

  const output = redact(input);

  assert.equal(output.includes("DEMO-BEARER-TOKEN-123456"), false);
  assert.equal(output.includes("demo-password"), false);
  assert.equal(output.includes("DEMO_TOKEN_PLAN_KEY_123456"), false);
  assert.equal(output.includes("DEMOHUGGINGFACETOKEN1234567890"), false);
  assert.equal(output.includes("DEMOGROQKEY12345678901234567890"), false);
  assert.equal(output.includes("DEMOXAIKEY12345678901234567890"), false);
  assert.match(output, /Bearer \[redacted-example\]|Authorization: \[redacted-example\]/);
  assert.match(output, /demo-user:\[redacted-password\]@example\.com/);
});

test("fails closed on raw and serialized credential-shaped evidence", () => {
  const bearer = ["Bearer", "DEMO-BEARER-TOKEN-123456"].join(" ");
  const tokenPlanKey = ["tp", "DEMO_TOKEN_PLAN_KEY_123456"].join("-");
  assert.throws(
    () => assertNoCredentialLikeText("api_key = generic-secret-value-123456"),
    /sensitive-assignment/,
  );
  assert.throws(
    () => assertNoCredentialLikeText(bearer),
    /bearer-token/,
  );
  assert.throws(
    () => serializeSnapshot({ text: "api_key = generic-secret-value-123456" }),
    /sensitive-assignment/,
  );
  assert.throws(
    () => serializeSnapshot({ text: tokenPlanKey }),
    /token-plan-key/,
  );

  const safe = redact(`api_key: str = "${demoSecret}"`);
  assert.doesNotThrow(() => serializeSnapshot({ text: safe }));
});

test("snippet extraction redacts a sensitive line without consuming the next line", () => {
  const content = [
    "model = demo-model",
    `api_key: str = "${demoSecret}"`,
    "return self.api_key",
    'api_key_env: "SAFE_ENV_NAME"',
  ].join("\n");

  const [snippet] = snippets(content);

  assert.equal(snippet.text.includes(demoSecret), false);
  assert.match(snippet.text, /^api_key: \[redacted-example\]$/m);
  assert.match(snippet.text, /^return self\.api_key$/m);
  assert.match(snippet.text, /^api_key_env: "SAFE_ENV_NAME"$/m);
});

test("normalizes fetch errors without persisting raw stderr", () => {
  assert.equal(
    safeFetchError({ code: "ETIMEDOUT", message: "secret raw stderr" }),
    "gh-api-failed:ETIMEDOUT",
  );
  assert.equal(
    safeFetchError({ message: "unsupported blob encoding: utf-8" }),
    "unsupported-blob-encoding",
  );
});
