import { expect, it } from "vitest";
import { formatError } from "./format";

it("formats structured command errors instead of showing object coercion", () => {
  expect(formatError({ message: "asset operation failed" })).toBe(
    "asset operation failed",
  );
  expect(formatError({ error: { detail: "rolled back" } })).toBe("rolled back");
  expect(formatError({ code: "unknown", retryable: false })).toBe(
    '{"code":"unknown","retryable":false}',
  );
});
