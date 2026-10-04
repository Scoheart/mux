import { describe, expect, it } from 'vitest';
import { mergeCaptureDelta, type CaptureDelta, type CaptureSnapshot, type FlowSummary } from './capture';

const flow = (id: string, started_at = 1): FlowSummary => ({
  id, started_at, method: 'GET', url: 'https://example.invalid/test', status: 200, duration_ms: 1, error: null,
});
const snapshot = (): CaptureSnapshot => ({
  session: { id: 'one', agent_id: 'codex', agent_name: 'Codex', target_id: 'fixture', target_name: 'Fixture',
    pids: [], proxy_url: null, started_at: '2026-10-05T00:00:00Z' },
  status: { state: 'running', message: '', pids: [], flow_count: 2 }, flows: [flow('newer', 2), flow('older')],
});
const delta = (current: CaptureSnapshot): CaptureDelta => ({
  session: current.session, status: current.status, revision: 'generation:1', reset: false, upserts: [], removed: [],
});

describe('mergeCaptureDelta', () => {
  it('preserves both snapshot and list references when polling an idle capture', () => {
    const current = snapshot();
    expect(mergeCaptureDelta(current, delta(current))).toBe(current);
  });

  it('merges updates and deletion/recreation without dropping unchanged rows', () => {
    const current = snapshot();
    const update = delta(current);
    update.removed = ['newer'];
    update.upserts = [{ ...flow('newer', 3), status: 404 }, flow('new', 4)];
    const next = mergeCaptureDelta(current, update);
    expect(next.flows.map(item => item.id)).toEqual(['new', 'newer', 'older']);
    expect(next.flows[1].status).toBe(404);
    expect(next.flows[2]).toBe(current.flows[1]);
  });

  it('replaces the list after cursor expiry or session replacement', () => {
    const current = snapshot();
    const update = delta(current);
    update.reset = true;
    update.upserts = [flow('replacement')];
    expect(mergeCaptureDelta(current, update).flows.map(item => item.id)).toEqual(['replacement']);
    update.reset = false;
    update.session = { ...current.session, id: 'other' };
    expect(mergeCaptureDelta(current, update).flows.map(item => item.id)).toEqual(['replacement']);
  });
});
