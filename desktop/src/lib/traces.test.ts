import { describe, expect, it } from "vitest";
import { traceMatches, type TraceEvent } from "./traces";

const call: TraceEvent = { id: "0.0", kind: "tool_call", timestamp: null, title: "act_ui", preview: '{"ref":"@e7"}', call_id: "call-123", is_error: false };
describe("trace preview filters", () => {
  it("matches tool names, previews and call IDs case-insensitively", () => {
    expect(traceMatches(call, "ACT_UI", "all")).toBe(true);
    expect(traceMatches(call, "@e7", "tools")).toBe(true);
    expect(traceMatches(call, "call-123", "tools")).toBe(true);
  });
  it("groups calls and results without grouping assistant messages", () => {
    expect(traceMatches({ ...call, kind: "tool_result" }, "", "tools")).toBe(true);
    expect(traceMatches({ ...call, kind: "assistant" }, "", "tools")).toBe(false);
    expect(traceMatches(call, "", "user")).toBe(false);
  });
  it("keeps unknown events separate and tolerates missing timestamps/IDs", () => {
    expect(traceMatches({ ...call, kind: "event", call_id: null }, "", "event")).toBe(true);
    expect(traceMatches({ ...call, call_id: null }, "missing", "all")).toBe(false);
  });
});
