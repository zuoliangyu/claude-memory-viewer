import {
  memo,
  startTransition,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  Clock3,
  Cpu,
  Loader2,
  Search,
  Wrench,
} from "lucide-react";
import { api } from "../../services/api";
import type {
  Trajectory,
  TrajectoryRecord,
  TrajectoryTokenUsage,
} from "../../types";
import {
  PERF_DIAGNOSTICS_ENABLED,
  getBrowserPerfSnapshot,
  nextPerfTraceId,
  recordPerfDiagnostic,
} from "../../utils/perfDiagnostics";

interface TrajectoryViewProps {
  source: string;
  filePath: string;
}

const RECORD_KIND_LABELS: Record<string, string> = {
  user: "用户",
  assistant: "助手",
  reasoning: "推理",
  tool: "工具",
  subagent: "子 Agent",
  compaction: "上下文压缩",
};
const TRAJECTORY_PAGE_SIZE = 80;

type TrajectoryRequestStage = "fast" | "full" | "earlier";

interface PendingTrajectoryCommit {
  requestId: string;
  stage: TrajectoryRequestStage;
  responseAt: number;
}

function getTrajectoryPerfFields(trajectory: Trajectory) {
  let detailChars = 0;
  let summaryChars = 0;
  for (const record of trajectory.records) {
    detailChars += (record.input?.length ?? 0) + (record.output?.length ?? 0);
    summaryChars += record.summary.length;
  }

  return {
    records: trajectory.records.length,
    turns: trajectory.turns.length,
    totalRecords: trajectory.stats.records,
    detailChars,
    summaryChars,
    approximateTextMb:
      Math.round(((detailChars + summaryChars) * 2 * 100) / 1024 / 1024) /
      100,
    complete: trajectory.pagination.complete,
  };
}

function mergeTrajectory(current: Trajectory, earlier: Trajectory): Trajectory {
  const records = Array.from(
    new Map(
      [...earlier.records, ...current.records].map((record) => [
        record.index,
        record,
      ]),
    ).values(),
  ).sort((left, right) => left.index - right.index);
  const turns = Array.from(
    new Map(
      [...earlier.turns, ...current.turns].map((turn) => [turn.index, turn]),
    ).values(),
  ).sort((left, right) => left.index - right.index);
  const warnings = Array.from(
    new Map(
      [...current.warnings, ...earlier.warnings].map((warning) => [
        `${warning.code}:${warning.line}:${warning.message}`,
        warning,
      ]),
    ).values(),
  );
  return {
    ...current,
    turns,
    records,
    warnings,
    stats: { ...current.stats, visibleRecords: records.length },
    pagination: {
      complete: current.pagination.complete && earlier.pagination.complete,
      firstRecord: records[0]?.index ?? null,
      lastRecord: records[records.length - 1]?.index ?? null,
      earlierRecords: earlier.pagination.earlierRecords,
      laterRecords: 0,
      hasEarlier: earlier.pagination.hasEarlier,
      hasLater: false,
      nextBeforeRecord: earlier.pagination.nextBeforeRecord,
    },
  };
}

function formatDuration(value: number | null | undefined): string {
  if (value == null) return "—";
  if (value < 1000) return `${value} ms`;
  return `${(value / 1000).toFixed(value < 10_000 ? 2 : 1)} s`;
}

function formatTime(value: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleTimeString([], {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      });
}

function formatTokens(value: number): string {
  return new Intl.NumberFormat("zh-CN", {
    notation: value >= 100_000 ? "compact" : "standard",
    maximumFractionDigits: 1,
  }).format(value);
}

function TokenStrip({ usage }: { usage: TrajectoryTokenUsage | null }) {
  if (!usage)
    return <span className="text-muted-foreground">暂无 Token 记录</span>;
  const input = Math.max(0, usage.inputTokens - usage.cachedInputTokens);
  const inputTotal = Math.max(1, usage.inputTokens);
  const output = Math.max(0, usage.outputTokens - usage.reasoningOutputTokens);
  const outputTotal = Math.max(1, usage.outputTokens);
  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-5">
        <Metric label="总 Token" value={formatTokens(usage.totalTokens)} />
        <Metric label="输入" value={formatTokens(usage.inputTokens)} />
        <Metric
          label="缓存读取"
          value={formatTokens(usage.cachedInputTokens)}
        />
        <Metric label="输出" value={formatTokens(usage.outputTokens)} />
        <Metric
          label="推理输出"
          value={formatTokens(usage.reasoningOutputTokens)}
        />
      </div>
      <div className="grid gap-2 sm:grid-cols-2">
        <TokenBar
          title="输入拆分"
          total={usage.inputTokens}
          left={input}
          right={usage.cachedInputTokens}
          leftLabel="未缓存"
          rightLabel="缓存"
          leftClass="bg-blue-500"
          rightClass="bg-teal-400"
          leftRatio={input / inputTotal}
        />
        <TokenBar
          title="输出拆分"
          total={usage.outputTokens}
          left={output}
          right={usage.reasoningOutputTokens}
          leftLabel="可见输出"
          rightLabel="推理"
          leftClass="bg-violet-500"
          rightClass="bg-fuchsia-400"
          leftRatio={output / outputTotal}
        />
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-border/70 bg-background/45 px-3 py-2">
      <div className="text-[10px] text-muted-foreground">{label}</div>
      <div className="mt-0.5 font-mono text-sm font-semibold tabular-nums">
        {value}
      </div>
    </div>
  );
}

function TokenBar({
  title,
  total,
  left,
  right,
  leftLabel,
  rightLabel,
  leftClass,
  rightClass,
  leftRatio,
}: {
  title: string;
  total: number;
  left: number;
  right: number;
  leftLabel: string;
  rightLabel: string;
  leftClass: string;
  rightClass: string;
  leftRatio: number;
}) {
  return (
    <div className="rounded-md border border-border/60 bg-muted/20 px-3 py-2">
      <div className="flex items-center justify-between text-[10px] text-muted-foreground">
        <span>{title}</span>
        <span className="font-mono">{formatTokens(total)}</span>
      </div>
      <div className="mt-2 flex h-2 overflow-hidden rounded-full bg-muted">
        <div
          className={leftClass}
          style={{ width: `${Math.max(0, Math.min(1, leftRatio)) * 100}%` }}
        />
        <div
          className={rightClass}
          style={{ width: `${Math.max(0, total ? right / total : 0) * 100}%` }}
        />
      </div>
      <div className="mt-1 flex justify-between text-[10px] text-muted-foreground">
        <span>
          <i
            className={`mr-1 inline-block h-1.5 w-1.5 rounded-full ${leftClass}`}
          />
          {leftLabel} {formatTokens(left)}
        </span>
        <span>
          <i
            className={`mr-1 inline-block h-1.5 w-1.5 rounded-full ${rightClass}`}
          />
          {rightLabel} {formatTokens(right)}
        </span>
      </div>
    </div>
  );
}

function recordColor(kind: string): string {
  switch (kind) {
    case "user":
      return "bg-blue-500";
    case "assistant":
      return "bg-violet-500";
    case "reasoning":
      return "bg-fuchsia-400";
    case "tool":
      return "bg-amber-500";
    case "subagent":
      return "bg-teal-400";
    case "compaction":
      return "bg-slate-400";
    default:
      return "bg-muted-foreground";
  }
}

function statusClass(status: string): string {
  if (status === "error") return "text-red-500";
  if (status === "running") return "text-amber-500";
  if (status === "aborted") return "text-orange-500";
  return "text-muted-foreground";
}

const TimingOverview = memo(function TimingOverview({
  records,
}: {
  records: TrajectoryRecord[];
}) {
  const timed = records
    .map((record) => {
      const start = new Date(
        record.startedAt || record.timestamp || "",
      ).getTime();
      const end = new Date(
        record.completedAt || record.timestamp || "",
      ).getTime();
      return { record, start, end: Math.max(start, end) };
    })
    .filter((item) => Number.isFinite(item.start) && Number.isFinite(item.end));
  if (timed.length === 0) return null;
  const min = Math.min(...timed.map((item) => item.start));
  const max = Math.max(...timed.map((item) => item.end), min + 1);
  const span = Math.max(1, max - min);
  return (
    <section className="rounded-lg border border-border bg-card p-3">
      <div className="mb-2 flex items-center justify-between text-sm font-semibold">
        <span className="flex items-center gap-1.5">
          <Clock3 className="h-4 w-4 text-primary" />
          耗时轴
        </span>
        <span className="font-mono text-xs font-normal text-muted-foreground">
          {formatDuration(span)}
        </span>
      </div>
      <div className="relative h-16 overflow-hidden rounded-md border border-border/70 bg-muted/20">
        <div className="absolute inset-x-0 top-1/2 border-t border-dashed border-border/70" />
        {timed.map(({ record, start, end }) => {
          const left = ((start - min) / span) * 100;
          const width = Math.max(1.2, ((end - start) / span) * 100);
          return (
            <div
              key={record.index}
              className={`absolute top-5 h-5 rounded-sm opacity-85 hover:opacity-100 ${recordColor(record.kind)}`}
              style={{
                left: `${left}%`,
                width: `${Math.min(width, 100 - left)}%`,
              }}
              title={`#${record.index} ${record.event} · ${formatDuration(record.durationMs)}`}
            />
          );
        })}
      </div>
      <div className="mt-1 flex justify-between text-[10px] text-muted-foreground">
        <span>{formatTime(new Date(min).toISOString())}</span>
        <span>{formatTime(new Date(max).toISOString())}</span>
      </div>
    </section>
  );
});

function RecordRow({
  record,
  selected,
  onSelect,
}: {
  record: TrajectoryRecord;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={`grid w-full grid-cols-[3.25rem_minmax(7rem,0.8fr)_minmax(0,2fr)_4.5rem] items-center gap-2 border-b border-border/50 px-3 py-2 text-left text-xs transition-colors hover:bg-accent/50 ${selected ? "bg-primary/10" : ""}`}
    >
      <span className="font-mono text-muted-foreground">#{record.index}</span>
      <span className="flex min-w-0 items-center gap-1.5">
        <i
          className={`h-2 w-2 shrink-0 rounded-full ${recordColor(record.kind)}`}
        />
        <span className="truncate font-medium">{record.event}</span>
      </span>
      <span className="truncate text-muted-foreground">{record.summary}</span>
      <span
        className={`text-right font-mono tabular-nums ${statusClass(record.status)}`}
      >
        {formatDuration(record.durationMs)}
      </span>
    </button>
  );
}

function RecordDetail({ record }: { record: TrajectoryRecord }) {
  return (
    <div className="border-b border-border bg-muted/20 px-4 py-3 text-xs">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-muted-foreground">
        <span>{record.kind}</span>
        <span>Turn {record.turn}</span>
        <span>Step {record.step ?? "—"}</span>
        <span>{formatTime(record.timestamp)}</span>
        {record.callId && (
          <span className="font-mono">call {record.callId}</span>
        )}
      </div>
      {(record.input || record.output) && (
        <div className="mt-3 grid gap-2 md:grid-cols-2">
          {record.input && <DetailBlock label="输入" value={record.input} />}
          {record.output && (
            <DetailBlock
              label="输出"
              value={record.output}
              error={record.status === "error"}
            />
          )}
        </div>
      )}
    </div>
  );
}

function DetailBlock({
  label,
  value,
  error,
}: {
  label: string;
  value: string;
  error?: boolean;
}) {
  return (
    <div
      className={`min-w-0 rounded-md border p-2 ${error ? "border-red-500/40 bg-red-500/5" : "border-border/70 bg-background/50"}`}
    >
      <div className="mb-1 text-[10px] font-medium text-muted-foreground">
        {label}
      </div>
      <pre className="max-h-56 overflow-auto whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed text-foreground">
        {value}
      </pre>
    </div>
  );
}

const TurnBlock = memo(function TurnBlock({
  turn,
  records,
}: {
  turn: Trajectory["turns"][number];
  records: TrajectoryRecord[];
}) {
  const [open, setOpen] = useState(false);
  const [selected, setSelected] = useState<number | null>(null);
  const selectedRecord =
    selected == null
      ? null
      : (records.find((record) => record.index === selected) ?? null);
  return (
    <section
      className="border-b border-border last:border-b-0"
      style={{
        contentVisibility: "auto",
        containIntrinsicSize: "auto 48px",
      }}
    >
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="flex w-full items-center gap-2 px-3 py-3 text-left hover:bg-accent/40"
      >
        {open ? (
          <ChevronDown className="h-4 w-4 shrink-0" />
        ) : (
          <ChevronRight className="h-4 w-4 shrink-0" />
        )}
        <span className="font-semibold">Turn {turn.index}</span>
        <span className={`text-xs ${statusClass(turn.status)}`}>
          {turn.status === "complete"
            ? "完成"
            : turn.status === "error"
              ? "失败"
              : turn.status === "aborted"
                ? "中止"
                : "进行中"}
        </span>
        <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">
          {turn.model || "未知模型"} · {turn.records} 条记录 ·{" "}
          {formatDuration(turn.durationMs)}
        </span>
        {turn.usage && (
          <span className="hidden font-mono text-xs text-muted-foreground sm:inline">
            {formatTokens(turn.usage.totalTokens)} tokens
          </span>
        )}
      </button>
      {open && (
        <div className="border-t border-border/60">
          <div className="hidden grid-cols-[3.25rem_minmax(7rem,0.8fr)_minmax(0,2fr)_4.5rem] gap-2 px-3 py-1.5 text-[10px] uppercase tracking-wide text-muted-foreground sm:grid">
            <span>Index</span>
            <span>Event</span>
            <span>Summary</span>
            <span className="text-right">Time</span>
          </div>
          {records.map((record) => (
            <div key={record.index}>
              <RecordRow
                record={record}
                selected={selected === record.index}
                onSelect={() =>
                  setSelected((value) =>
                    value === record.index ? null : record.index,
                  )
                }
              />
              {selectedRecord?.index === record.index && (
                <RecordDetail record={record} />
              )}
            </div>
          ))}
        </div>
      )}
    </section>
  );
});

export function TrajectoryView({ source, filePath }: TrajectoryViewProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const pendingCommitRef = useRef<PendingTrajectoryCommit | null>(null);
  const [trajectory, setTrajectory] = useState<Trajectory | null>(null);
  const [loading, setLoading] = useState(true);
  const [enriching, setEnriching] = useState(false);
  const [loadingEarlier, setLoadingEarlier] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [enrichmentError, setEnrichmentError] = useState<string | null>(null);
  const [pagingError, setPagingError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState("all");

  const requestTrajectory = useCallback(
    async (
      stage: TrajectoryRequestStage,
      beforeRecord?: number,
      fast = false,
    ) => {
      const requestId = nextPerfTraceId(`trajectory-${stage}`);
      const startedAt = performance.now();
      recordPerfDiagnostic("trajectory.request_started", undefined, {
        requestId,
        stage,
        fast,
        maxRecords: TRAJECTORY_PAGE_SIZE,
        beforeRecord: beforeRecord ?? null,
      });

      try {
        const result = await api.getTrajectory(
          source,
          filePath,
          TRAJECTORY_PAGE_SIZE,
          beforeRecord,
          fast,
        );
        const responseAt = performance.now();
        recordPerfDiagnostic(
          "trajectory.ipc_roundtrip",
          responseAt - startedAt,
          {
            requestId,
            stage,
            fast,
            ...getTrajectoryPerfFields(result),
          },
        );
        return { result, requestId, responseAt };
      } catch (reason: unknown) {
        recordPerfDiagnostic(
          "trajectory.request_error",
          performance.now() - startedAt,
          {
            requestId,
            stage,
            fast,
            errorType: reason instanceof Error ? reason.name : typeof reason,
          },
        );
        throw reason;
      }
    },
    [filePath, source],
  );

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setEnriching(false);
    setTrajectory(null);
    setError(null);
    setEnrichmentError(null);
    setPagingError(null);
    setQuery("");
    setKind("all");
    const load = async () => {
      try {
        const fastResponse = await requestTrajectory("fast", undefined, true);
        if (cancelled) return;
        const fastResult = fastResponse.result;
        pendingCommitRef.current = {
          requestId: fastResponse.requestId,
          stage: "fast",
          responseAt: fastResponse.responseAt,
        };
        setTrajectory(fastResult);
        setLoading(false);

        if (fastResult.pagination.complete) return;
        setEnriching(true);
        try {
          const fullResponse = await requestTrajectory("full");
          if (!cancelled) {
            pendingCommitRef.current = {
              requestId: fullResponse.requestId,
              stage: "full",
              responseAt: fullResponse.responseAt,
            };
            startTransition(() => setTrajectory(fullResponse.result));
          }
        } catch (reason: unknown) {
          if (!cancelled) {
            setEnrichmentError(
              reason instanceof Error ? reason.message : String(reason),
            );
          }
        } finally {
          if (!cancelled) setEnriching(false);
        }
      } catch (reason: unknown) {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    void load();
    return () => {
      cancelled = true;
    };
  }, [requestTrajectory]);

  const availableKinds = useMemo(
    () =>
      Array.from(
        new Set(trajectory?.records.map((record) => record.kind) ?? []),
      ).sort(),
    [trajectory],
  );

  const filteredRecords = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    return (trajectory?.records ?? []).filter((record) => {
      if (kind !== "all" && record.kind !== kind) return false;
      if (!normalizedQuery) return true;
      return [record.event, record.summary, record.input, record.output]
        .filter((value): value is string => Boolean(value))
        .some((value) => value.toLocaleLowerCase().includes(normalizedQuery));
    });
  }, [kind, query, trajectory]);

  const recordsByTurn = useMemo(() => {
    const groups = new Map<number, TrajectoryRecord[]>();
    filteredRecords.forEach((record) => {
      const list = groups.get(record.turn) ?? [];
      list.push(record);
      groups.set(record.turn, list);
    });
    return groups;
  }, [filteredRecords]);

  const visibleTurns = useMemo(
    () =>
      trajectory?.turns.filter((turn) => recordsByTurn.has(turn.index)) ?? [],
    [recordsByTurn, trajectory],
  );

  useLayoutEffect(() => {
    if (!PERF_DIAGNOSTICS_ENABLED || !trajectory || !rootRef.current) return;

    const root = rootRef.current;
    const pendingCommit = pendingCommitRef.current;
    pendingCommitRef.current = null;
    const committedAt = performance.now();
    const commonFields = {
      requestId: pendingCommit?.requestId ?? null,
      stage: pendingCommit?.stage ?? "state-update",
      ...getTrajectoryPerfFields(trajectory),
      ...getBrowserPerfSnapshot(root),
    };

    recordPerfDiagnostic(
      "trajectory.dom_committed",
      pendingCommit ? committedAt - pendingCommit.responseAt : undefined,
      commonFields,
    );

    let secondFrame = 0;
    const firstFrame = requestAnimationFrame(() => {
      secondFrame = requestAnimationFrame(() => {
        if (!root.isConnected) return;
        recordPerfDiagnostic(
          "trajectory.paint_ready",
          performance.now() - committedAt,
          {
            ...commonFields,
            ...getBrowserPerfSnapshot(root),
          },
        );
      });
    });

    return () => {
      cancelAnimationFrame(firstFrame);
      if (secondFrame) cancelAnimationFrame(secondFrame);
    };
  }, [trajectory]);

  const loadEarlier = async () => {
    if (
      !trajectory?.pagination.complete ||
      !trajectory.pagination.nextBeforeRecord ||
      loadingEarlier
    )
      return;
    setLoadingEarlier(true);
    setPagingError(null);
    try {
      const earlierResponse = await requestTrajectory(
        "earlier",
        trajectory.pagination.nextBeforeRecord,
      );
      pendingCommitRef.current = {
        requestId: earlierResponse.requestId,
        stage: "earlier",
        responseAt: earlierResponse.responseAt,
      };
      setTrajectory((current) =>
        current
          ? mergeTrajectory(current, earlierResponse.result)
          : earlierResponse.result,
      );
    } catch (reason: unknown) {
      setPagingError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLoadingEarlier(false);
    }
  };

  if (loading)
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
        加载轨迹中...
      </div>
    );
  if (error)
    return (
      <div className="mx-auto mt-8 flex max-w-xl items-start gap-2 rounded-lg border border-red-500/30 bg-red-500/5 p-4 text-sm text-red-500">
        <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
        <span className="break-words">{error}</span>
      </div>
    );
  if (!trajectory) return null;

  return (
    <div ref={rootRef} className="p-3 sm:p-4">
      <div className="mx-auto max-w-6xl space-y-3">
        <div className="flex flex-wrap items-start justify-between gap-3 border-b border-border pb-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <Clock3 className="h-4 w-4 text-primary" />
              轨迹摘要
            </div>
            <div className="mt-1 truncate text-xs text-muted-foreground">
              {trajectory.session.title || trajectory.session.id} ·{" "}
              {trajectory.session.model || "未知模型"}
            </div>
          </div>
          <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
            <span>
              {trajectory.stats.turns} turns
              {!trajectory.pagination.complete && "（当前片段）"}
            </span>
            <span>
              {trajectory.stats.records} records
              {!trajectory.pagination.complete && "（当前片段）"}
            </span>
            <span>
              <Wrench className="mr-1 inline h-3 w-3" />
              {trajectory.stats.toolCalls} 工具
            </span>
            <span>{formatDuration(trajectory.stats.durationMs)}</span>
          </div>
        </div>
        {(enriching || enrichmentError) && (
          <div
            className={`flex items-center gap-2 rounded-md border px-3 py-2 text-xs ${
              enrichmentError
                ? "border-yellow-500/40 bg-yellow-500/5 text-yellow-600 dark:text-yellow-400"
                : "border-border bg-muted/30 text-muted-foreground"
            }`}
          >
            {enriching ? (
              <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin" />
            ) : (
              <AlertTriangle className="h-3.5 w-3.5 shrink-0" />
            )}
            <span
              className="min-w-0 truncate"
              title={enrichmentError ?? undefined}
            >
              {enrichmentError
                ? "全量轨迹补全失败，当前显示最近片段"
                : "正在后台补全全局统计与早期轨迹..."}
            </span>
          </div>
        )}
        <section className="rounded-lg border border-border bg-card p-3">
          <TokenStrip usage={trajectory.stats.tokens} />
        </section>
        <TimingOverview records={filteredRecords} />
        {trajectory.warnings.length > 0 && (
          <section className="rounded-lg border border-yellow-500/40 bg-yellow-500/5 p-3 text-xs text-yellow-600 dark:text-yellow-400">
            <div className="flex items-center gap-1.5 font-medium">
              <AlertTriangle className="h-3.5 w-3.5" />
              {trajectory.warnings.length} 条解析警告
            </div>
            <div className="mt-1 space-y-0.5 text-muted-foreground">
              {trajectory.warnings.slice(0, 5).map((warning, index) => (
                <div key={`${warning.code}-${index}`} className="truncate">
                  {warning.message}
                </div>
              ))}
            </div>
          </section>
        )}
        <section className="overflow-hidden rounded-lg border border-border bg-card">
          <div className="border-b border-border p-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="flex items-center gap-1.5 text-sm font-semibold">
                <Cpu className="h-4 w-4 text-primary" />
                事件账本
              </span>
              <span className="text-xs text-muted-foreground">
                {trajectory.pagination.complete
                  ? `已加载 ${trajectory.records.length} / ${trajectory.stats.records} 条`
                  : `当前片段 ${trajectory.records.length} 条`}
              </span>
            </div>
            <div className="mt-2 flex flex-col gap-2 sm:flex-row">
              <label className="relative min-w-0 flex-1">
                <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
                <input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="搜索事件、摘要或详情"
                  className="h-8 w-full rounded-md border border-border bg-background pl-8 pr-2 text-xs outline-none focus:border-primary"
                />
              </label>
              <select
                value={kind}
                onChange={(event) => setKind(event.target.value)}
                className="h-8 rounded-md border border-border bg-background px-2 text-xs outline-none focus:border-primary"
                aria-label="筛选事件类型"
              >
                <option value="all">全部类型</option>
                {availableKinds.map((value) => (
                  <option key={value} value={value}>
                    {RECORD_KIND_LABELS[value] ?? value}
                  </option>
                ))}
              </select>
            </div>
            {trajectory.pagination.complete &&
              trajectory.pagination.hasEarlier && (
                <div className="mt-2 flex items-center gap-2">
                  <button
                    type="button"
                    onClick={loadEarlier}
                    disabled={loadingEarlier}
                    className="inline-flex h-8 items-center gap-1.5 rounded-md border border-border px-2.5 text-xs hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    {loadingEarlier && (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    )}
                    加载更早记录（剩余 {trajectory.pagination.earlierRecords}{" "}
                    条）
                  </button>
                  {pagingError && (
                    <span
                      className="min-w-0 truncate text-xs text-red-500"
                      title={pagingError}
                    >
                      {pagingError}
                    </span>
                  )}
                </div>
              )}
          </div>
          {visibleTurns.length === 0 ? (
            <div className="p-8 text-center text-sm text-muted-foreground">
              {trajectory.records.length === 0
                ? "未发现可展示的轨迹事件"
                : "没有符合筛选条件的记录"}
            </div>
          ) : (
            visibleTurns.map((turn) => (
              <TurnBlock
                key={turn.index}
                turn={turn}
                records={recordsByTurn.get(turn.index) ?? []}
              />
            ))
          )}
        </section>
      </div>
    </div>
  );
}
