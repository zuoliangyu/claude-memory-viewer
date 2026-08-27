import { useEffect, useState } from "react";
import { DollarSign, X, Copy, Check, Receipt } from "lucide-react";
import { useAppStore } from "../../stores/appStore";
import type { RequestRecord, SessionCostSummary } from "../../types";
import { formatShortDateTime } from "../../utils/dateTime";

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}

function formatCost(usd: number): string {
  if (usd >= 100) return `$${usd.toFixed(0)}`;
  if (usd >= 1) return `$${usd.toFixed(2)}`;
  if (usd >= 0.01) return `$${usd.toFixed(3)}`;
  if (usd > 0) return `$${usd.toFixed(4)}`;
  return "$0";
}

function formatDuration(ms: number | null): string {
  if (ms === null || ms === undefined) return "—";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const mins = Math.floor(ms / 60_000);
  const secs = ((ms % 60_000) / 1000).toFixed(0);
  return `${mins}m${secs}s`;
}

/**
 * Header chip that summarises a session's cumulative cost. Clicking it
 * opens a modal with a per-request table that the user can copy as a
 * Markdown table — useful for sharing receipts in a team chat.
 */
export function SessionCostBadge({ filePath }: { filePath: string }) {
  const summary = useAppStore((state) => state.sessionCosts[filePath] ?? null);
  const messagesLoading = useAppStore((state) => state.messagesLoading);
  const loadSessionCost = useAppStore((state) => state.loadSessionCost);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (filePath && !messagesLoading) {
      void loadSessionCost(filePath);
    }
  }, [filePath, loadSessionCost, messagesLoading]);

  if (!summary || summary.requestCount === 0) {
    // Nothing useful to show yet — for very fresh sessions or for sources
    // we couldn't extract usage from. Don't crowd the header with a zero.
    return null;
  }
  const isFullyPriced = summary.requests.every((request) => request.isPriced);

  return (
    <>
      <button
        onClick={() => setOpen(true)}
        className="hidden md:flex items-center gap-1 px-2 py-1 text-[11px] rounded border border-green-500/30 bg-green-500/5 text-green-600 dark:text-green-400 hover:bg-green-500/10 transition-colors font-mono"
        title="点击查看本会话的逐请求账单"
      >
        <DollarSign className="w-3 h-3" />
        <span>{isFullyPriced ? formatCost(summary.costUsd) : "未定价"}</span>
        <span className="text-muted-foreground text-[10px] ml-1">
          · {summary.requestCount} req
        </span>
      </button>

      {open && (
        <SessionCostModal summary={summary} onClose={() => setOpen(false)} />
      )}
    </>
  );
}

function SessionCostModal({
  summary,
  onClose,
}: {
  summary: SessionCostSummary;
  onClose: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const timeZone = useAppStore((state) => state.timeZone);
  const isFullyPriced = summary.requests.every((request) => request.isPriced);

  const handleCopy = async () => {
    const md = buildMarkdownTable(summary, timeZone);
    try {
      await navigator.clipboard.writeText(md);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      console.error("clipboard write failed", e);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={onClose}
    >
      <div
        className="bg-card border border-border rounded-lg shadow-lg w-[56rem] max-w-[95vw] max-h-[85vh] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-border">
          <div className="flex items-center gap-2">
            <Receipt className="w-4 h-4 text-muted-foreground" />
            <h2 className="text-sm font-semibold">本会话账单</h2>
            <span className="text-xs text-muted-foreground">
              · {summary.requestCount} 次请求 · 平均{" "}
              {isFullyPriced && summary.avgCostUsd !== null
                ? formatCost(summary.avgCostUsd)
                : "未定价"}/次
            </span>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={handleCopy}
              className="flex items-center gap-1 px-2.5 py-1 text-xs rounded border border-border hover:bg-accent transition-colors"
              title="复制为 Markdown 表格"
            >
              {copied ? (
                <>
                  <Check className="w-3 h-3 text-green-500" />
                  已复制
                </>
              ) : (
                <>
                  <Copy className="w-3 h-3" />
                  复制 Markdown
                </>
              )}
            </button>
            <button
              onClick={onClose}
              className="p-1 rounded transition-colors text-muted-foreground hover:text-foreground hover:bg-accent/50"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Summary row */}
        <div className="grid grid-cols-5 gap-2 px-4 py-3 border-b border-border bg-muted/30 text-xs">
          <SummaryStat
            label="累计花费"
            value={isFullyPriced ? formatCost(summary.costUsd) : "未定价"}
            accent="text-green-500"
          />
          <SummaryStat label="输入" value={formatTokens(summary.inputTokens)} />
          <SummaryStat label="缓存读" value={formatTokens(summary.cacheReadTokens)} accent="text-teal-500" />
          <SummaryStat label="缓存写" value={formatTokens(summary.cacheCreationTokens)} accent="text-purple-500" />
          <SummaryStat label="输出" value={formatTokens(summary.outputTokens)} accent="text-amber-500" />
        </div>

        {/* Per-request rows */}
        <div className="flex-1 overflow-auto">
          <table className="w-full text-xs">
            <thead className="sticky top-0 bg-muted/40 border-b border-border">
              <tr className="text-[11px] text-muted-foreground">
                <th className="text-left font-medium px-3 py-2">时间</th>
                <th className="text-left font-medium px-3 py-2">模型</th>
                <th className="text-right font-medium px-3 py-2">input</th>
                <th className="text-right font-medium px-3 py-2">cache 读</th>
                <th className="text-right font-medium px-3 py-2">cache 写</th>
                <th className="text-right font-medium px-3 py-2">output</th>
                <th className="text-right font-medium px-3 py-2">耗时</th>
                <th className="text-right font-medium px-3 py-2">花费</th>
              </tr>
            </thead>
            <tbody>
              {summary.requests.map((r, i) => (
                <tr
                  key={`${r.timestamp}-${i}`}
                  className="border-b border-border/40 hover:bg-accent/30 transition-colors"
                >
                  <td className="px-3 py-1.5 font-mono text-muted-foreground">
                    {r.timestamp ? formatShortDateTime(r.timestamp, timeZone) : "—"}
                  </td>
                  <td className="px-3 py-1.5 font-mono text-[10.5px] truncate max-w-[14rem]">
                    {r.model}
                  </td>
                  <td className="px-3 py-1.5 text-right font-mono">
                    {formatTokens(r.inputTokens)}
                  </td>
                  <td className="px-3 py-1.5 text-right font-mono text-teal-500">
                    {r.cacheReadTokens > 0 ? formatTokens(r.cacheReadTokens) : "—"}
                  </td>
                  <td className="px-3 py-1.5 text-right font-mono text-purple-500">
                    {r.cacheCreationTokens > 0 ? formatTokens(r.cacheCreationTokens) : "—"}
                  </td>
                  <td className="px-3 py-1.5 text-right font-mono text-amber-500">
                    {formatTokens(r.outputTokens)}
                  </td>
                  <td className="px-3 py-1.5 text-right font-mono text-muted-foreground">
                    {formatDuration(r.durationMs)}
                  </td>
                  <td className="px-3 py-1.5 text-right font-mono text-green-500">
                    {r.isPriced ? formatCost(r.costUsd) : "未定价"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function SummaryStat({
  label,
  value,
  accent,
}: {
  label: string;
  value: string;
  accent?: string;
}) {
  return (
    <div>
      <div className="text-[10px] uppercase tracking-wider text-muted-foreground">
        {label}
      </div>
      <div className={`font-mono text-sm font-semibold ${accent ?? ""}`}>{value}</div>
    </div>
  );
}

function buildMarkdownTable(summary: SessionCostSummary, timeZone: string): string {
  const header =
    "| 时间 | 模型 | input | cache 读 | cache 写 | output | 耗时 | 花费 |\n" +
    "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |\n";
  const rows = summary.requests
    .map((r) => buildRow(r, timeZone))
    .join("\n");
  const footer =
    `\n\n**累计**: ${summary.requestCount} 次请求 · ${summary.requests.every((r) => r.isPriced) ? formatCost(summary.costUsd) : "未定价"}` +
    ` · input ${formatTokens(summary.inputTokens)}` +
    ` · cache 读 ${formatTokens(summary.cacheReadTokens)}` +
    ` · cache 写 ${formatTokens(summary.cacheCreationTokens)}` +
    ` · output ${formatTokens(summary.outputTokens)}`;
  return header + rows + footer;
}

function buildRow(r: RequestRecord, timeZone: string): string {
  return (
    `| ${r.timestamp ? formatShortDateTime(r.timestamp, timeZone) : "—"} | \`${r.model}\` | ${formatTokens(r.inputTokens)} | ` +
    `${r.cacheReadTokens > 0 ? formatTokens(r.cacheReadTokens) : "—"} | ` +
    `${r.cacheCreationTokens > 0 ? formatTokens(r.cacheCreationTokens) : "—"} | ` +
    `${formatTokens(r.outputTokens)} | ${formatDuration(r.durationMs)} | ${r.isPriced ? formatCost(r.costUsd) : "未定价"} |`
  );
}
