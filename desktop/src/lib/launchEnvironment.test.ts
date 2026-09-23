import { expect, it } from "vitest";
import { formatLaunchEnvironment, parseLaunchEnvironment } from "./launchEnvironment";

it("parses a pasted block, including export and quotes", () => {
  const parsed = parseLaunchEnvironment(`
# proxy
export HTTP_PROXY="http://127.0.0.1:6789"
http_proxy=http://127.0.0.1:6789
NO_PROXY='localhost,127.0.0.1,::1'
`);
  expect(parsed.error).toBeNull();
  expect(parsed.env).toEqual({
    HTTP_PROXY: "http://127.0.0.1:6789",
    NO_PROXY: "localhost,127.0.0.1,::1",
    http_proxy: "http://127.0.0.1:6789",
  });
});

it("keeps equals signs that belong to the value", () => {
  expect(parseLaunchEnvironment("TOKEN=abc=def").env).toEqual({ TOKEN: "abc=def" });
});

it("reports the first line that is not NAME=VALUE", () => {
  expect(parseLaunchEnvironment("HTTP_PROXY=http://127.0.0.1:6789\nnot a pair").error).toBe("第 2 行需要 NAME=VALUE");
});

it("round-trips a saved environment map", () => {
  const env = { ALL_PROXY: "http://127.0.0.1:6789", NO_PROXY: "localhost" };
  expect(parseLaunchEnvironment(formatLaunchEnvironment(env)).env).toEqual(env);
});
