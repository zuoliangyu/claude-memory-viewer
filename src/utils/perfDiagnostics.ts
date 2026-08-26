import type { ProfilerOnRenderCallback } from "react";
import type { DisplayMessage } from "../types";

declare const __IS_TAURI__: boolean;

type PerfFieldValue = string | number | boolean | null;

interface PerfDiagnosticEvent {
  timestamp: string;
  name: string;
  durationMs?: number;
  fields: Record<string, PerfFieldValue>;
}

interface ChromiumPerformanceMemory {
  usedJSHeapSize: number;
  totalJSHeapSize: number;
  jsHeapSizeLimit: number;
}

export type MessageRequestStage =
  | "initial"
  | "older"
  | "newer"
  | "jump"
  | "reload"
  | "background";

export interface PendingMessageCommit {
  requestId: string;
  stage: MessageRequestStage;
  responseAt: number;
}

const FLUSH_DELAY_MS = 500;
const MAX_QUEUE_SIZE = 200;
const MAX_BATCH_SIZE = 40;
const LONG_TASK_THRESHOLD_MS = 50;
const EVENT_LOOP_INTERVAL_MS = 1_000;
const EVENT_LOOP_LAG_THRESHOLD_MS = 100;

export const PERF_DIAGNOSTICS_ENABLED =
  import.meta.env.DEV &&
  import.meta.env.VITE_ASV_PERF_DIAGNOSTICS === "1";

let initialized = false;
let traceSequence = 0;
let flushTimer: number | null = null;
let flushing = false;
let droppedEvents = 0;
const eventQueue: PerfDiagnosticEvent[] = [];
let pendingMessageCommit: PendingMessageCommit | null = null;

function round(value: number): number {
  return Math.round(value * 100) / 100;
}

function scheduleFlush(): void {
  if (flushTimer != null || flushing) return;
  flushTimer = window.setTimeout(() => {
    flushTimer = null;
    void flushPerfDiagnostics();
  }, FLUSH_DELAY_MS);
}

async function flushPerfDiagnostics(): Promise<void> {
  if (!PERF_DIAGNOSTICS_ENABLED || flushing || eventQueue.length === 0) {
    return;
  }

  flushing = true;
  const batch = eventQueue.splice(0, MAX_BATCH_SIZE);
  if (droppedEvents > 0) {
    batch.unshift({
      timestamp: new Date().toISOString(),
      name: "diagnostics.events_dropped",
      fields: { count: droppedEvents },
    });
    droppedEvents = 0;
  }

  try {
    if (__IS_TAURI__) {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("report_perf_events", { events: batch });
    } else {
      console.info("[ASV-PERF]", batch);
    }
  } catch (reason) {
    console.warn("[ASV-PERF] 性能事件上报失败", reason);
  } finally {
    flushing = false;
    if (eventQueue.length > 0) scheduleFlush();
  }
}

export function recordPerfDiagnostic(
  name: string,
  durationMs?: number,
  fields: Record<string, PerfFieldValue> = {},
): void {
  if (!PERF_DIAGNOSTICS_ENABLED) return;

  if (eventQueue.length >= MAX_QUEUE_SIZE) {
    eventQueue.shift();
    droppedEvents += 1;
  }

  eventQueue.push({
    timestamp: new Date().toISOString(),
    name,
    ...(durationMs == null ? {} : { durationMs: round(durationMs) }),
    fields,
  });
  scheduleFlush();
}

export function nextPerfTraceId(prefix: string): string {
  traceSequence += 1;
  return `${prefix}-${traceSequence}`;
}

export function getBrowserPerfSnapshot(
  root?: HTMLElement | null,
): Record<string, PerfFieldValue> {
  const memory = (
    performance as Performance & { memory?: ChromiumPerformanceMemory }
  ).memory;

  return {
    documentNodes: document.getElementsByTagName("*").length,
    rootNodes: root?.getElementsByTagName("*").length ?? null,
    usedHeapMb: memory ? round(memory.usedJSHeapSize / 1024 / 1024) : null,
    totalHeapMb: memory ? round(memory.totalJSHeapSize / 1024 / 1024) : null,
    heapLimitMb: memory ? round(memory.jsHeapSizeLimit / 1024 / 1024) : null,
  };
}

function getMessageBlockChars(message: DisplayMessage): number {
  return message.content.reduce((total, block) => {
    switch (block.type) {
      case "text":
      case "reasoning":
        return total + block.text.length;
      case "thinking":
        return total + block.thinking.length;
      case "tool_use":
        return total + block.input.length;
      case "tool_result":
        return total + block.content.length;
      case "function_call":
        return total + block.arguments.length;
      case "function_call_output":
        return total + block.output.length;
    }
  }, 0);
}

export function getMessagesPerfFields(
  messages: DisplayMessage[],
): Record<string, PerfFieldValue> {
  let blocks = 0;
  let textChars = 0;
  for (const message of messages) {
    blocks += message.content.length;
    textChars += getMessageBlockChars(message);
  }

  return {
    messages: messages.length,
    blocks,
    textChars,
    approximateTextMb: round((textChars * 2) / 1024 / 1024),
  };
}

export function markPendingMessageCommit(
  commit: PendingMessageCommit,
): void {
  if (!PERF_DIAGNOSTICS_ENABLED) return;
  pendingMessageCommit = commit;
}

export function consumePendingMessageCommit(): PendingMessageCommit | null {
  if (!PERF_DIAGNOSTICS_ENABLED) return null;
  const commit = pendingMessageCommit;
  pendingMessageCommit = null;
  return commit;
}

export const recordPerfProfilerRender: ProfilerOnRenderCallback = (
  id,
  phase,
  actualDuration,
  baseDuration,
  startTime,
  commitTime,
) => {
  recordPerfDiagnostic("react.commit", actualDuration, {
    id,
    phase,
    baseDurationMs: round(baseDuration),
    startTimeMs: round(startTime),
    commitTimeMs: round(commitTime),
  });
};

export const recordMessagesProfilerRender: ProfilerOnRenderCallback = (
  id,
  phase,
  actualDuration,
  baseDuration,
  startTime,
  commitTime,
) => {
  recordPerfDiagnostic("messages.react_commit", actualDuration, {
    id,
    phase,
    baseDurationMs: round(baseDuration),
    startTimeMs: round(startTime),
    commitTimeMs: round(commitTime),
  });
};

export function initializePerfDiagnostics(): void {
  if (!PERF_DIAGNOSTICS_ENABLED || initialized) return;
  initialized = true;

  recordPerfDiagnostic("diagnostics.started", undefined, {
    url: window.location.pathname,
    visibility: document.visibilityState,
  });

  try {
    const observer = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        if (entry.duration < LONG_TASK_THRESHOLD_MS) continue;
        recordPerfDiagnostic("browser.long_task", entry.duration, {
          startTimeMs: round(entry.startTime),
          entryType: entry.entryType,
        });
      }
    });
    observer.observe({ type: "longtask", buffered: true });
  } catch (reason) {
    recordPerfDiagnostic("diagnostics.long_task_unavailable", undefined, {
      reason: reason instanceof Error ? reason.message : String(reason),
    });
  }

  let expectedTick = performance.now() + EVENT_LOOP_INTERVAL_MS;
  window.setInterval(() => {
    const now = performance.now();
    const lag = now - expectedTick;
    expectedTick = now + EVENT_LOOP_INTERVAL_MS;
    if (
      document.visibilityState === "visible" &&
      lag >= EVENT_LOOP_LAG_THRESHOLD_MS
    ) {
      recordPerfDiagnostic("browser.event_loop_lag", lag, {
        visibility: document.visibilityState,
      });
    }
  }, EVENT_LOOP_INTERVAL_MS);

  window.addEventListener("pagehide", () => {
    void flushPerfDiagnostics();
  });
}
