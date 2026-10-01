import { invoke } from '@tauri-apps/api/core';
export interface CaptureTarget { id: string; name: string; pids: number[]; agent_ids: string[] }
export interface CaptureEnvironment { supported: boolean; engine: string | null; extension_enabled: boolean; certificate_present: boolean; certificate_trusted: boolean; targets: CaptureTarget[] }
export interface CaptureSession { id: string; agent_id: string; agent_name: string; target_id: string; target_name: string; pids: number[]; proxy_url: string | null; started_at: string }
export interface CaptureStatus { state: string; message: string; pids: number[]; flow_count: number }
export interface FlowSummary { id: string; method: string; url: string; status: number | null; started_at: number; duration_ms: number | null; error: string | null }
export interface CapturedMessage { headers: [string, string][]; body: string; bytes: number; truncated: boolean; encoding: string }
export interface CapturedFlow extends FlowSummary { request: CapturedMessage | null; response: CapturedMessage | null; request_http_version: string; response_http_version: string | null; proxy_url: string | null }
export interface CaptureSnapshot { session: CaptureSession; status: CaptureStatus; flows: FlowSummary[] }
export const captureEnvironment = () => invoke<CaptureEnvironment>('capture_environment');
export const captureSessions = () => invoke<CaptureSession[]>('capture_sessions');
export const captureSnapshot = (sessionId: string) => invoke<CaptureSnapshot>('capture_snapshot', { sessionId });
export const captureDetail = (sessionId: string, flowId: string) => invoke<CapturedFlow>('capture_detail', { sessionId, flowId });
export const captureStart = (request: { agent_id: string; target_id: string; egress: string; proxy_url: string | null }) => invoke<CaptureSnapshot>('capture_start', { request });
export const captureStop = (sessionId: string) => invoke<CaptureSnapshot>('capture_stop', { sessionId });
export const captureExport = (sessionId: string, flowId?: string) => invoke<string | null>('capture_export', { sessionId, flowId: flowId ?? null });
