import { memo, useCallback, useMemo, useState } from "react";
import {
  MessageCircleQuestion,
  CornerDownRight,
  GitBranch,
  Loader2,
  Check,
} from "lucide-react";
import type { QuestionIndexEntry } from "../../types";
import { formatTime } from "./utils";
import { useAppStore } from "../../stores/appStore";
import { api } from "../../services/api";

interface ThreadSummaryViewProps {
  questions: QuestionIndexEntry[];
  source: string;
  onSelect: (messageId: string) => void;
  filePath?: string;
  projectPath?: string;
}

interface UserThreadItem {
  messageId: string;
  messageIndex: number;
  question: string;
  timestamp: string | null;
  replyPreview: string;
  replyModel: string | null;
  replyTimestamp: string | null;
  hasTool: boolean;
  parentMessageId: string | null;
  depth: number;
  branchIndex: number;
  branchCount: number;
}

function flattenQuestionIndex(
  questions: QuestionIndexEntry[],
  timeZone: string,
): UserThreadItem[] {
  const byMessageIndex = new Map(questions.map((question) => [question.messageIndex, question]));
  const childCounts = new Map<number, number>();
  const items: UserThreadItem[] = [];

  for (const question of questions) {
    const parentMessageIndex = question.parentMessageIndex;
    const branchIndex = parentMessageIndex === null
      ? 0
      : childCounts.get(parentMessageIndex) ?? 0;
    if (parentMessageIndex !== null) {
      childCounts.set(parentMessageIndex, branchIndex + 1);
    }

    let depth = 0;
    let ancestor = parentMessageIndex;
    const visited = new Set<number>();
    while (ancestor !== null && !visited.has(ancestor)) {
      visited.add(ancestor);
      const parent = byMessageIndex.get(ancestor);
      if (!parent) break;
      depth += 1;
      ancestor = parent.parentMessageIndex;
    }

    items.push({
      messageId: question.messageId,
      messageIndex: question.messageIndex,
      question: question.preview || "（用户消息）",
      timestamp: question.timestamp ? formatTime(question.timestamp, timeZone) : null,
      replyPreview: question.replyPreview,
      replyModel: question.replyModel,
      replyTimestamp: question.replyTimestamp
        ? formatTime(question.replyTimestamp, timeZone)
        : null,
      hasTool: question.hasTool,
      parentMessageId: parentMessageIndex === null
        ? null
        : byMessageIndex.get(parentMessageIndex)?.messageId ?? null,
      depth,
      branchIndex,
      branchCount: 0,
    });
  }

  for (const item of items) {
    item.branchCount = childCounts.get(item.messageIndex) ?? 0;
  }
  return items;
}

export const ThreadSummaryView = memo(function ThreadSummaryView({
  questions,
  source,
  onSelect,
  filePath,
  projectPath,
}: ThreadSummaryViewProps) {
  const timeZone = useAppStore((state) => state.timeZone);
  const items = useMemo(
    () => flattenQuestionIndex(questions, timeZone),
    [questions, timeZone],
  );
  const isThreaded = useMemo(
    () => questions.some((question) => question.parentMessageIndex !== null),
    [questions],
  );
  const assistantName = source === "codex" ? "Codex" : source === "omp" ? "Oh My Pi" : "Claude";

  const terminalShell = useAppStore((state) => state.terminalShell);
  const refreshInBackground = useAppStore((state) => state.refreshInBackground);
  const [forkingMsgId, setForkingMsgId] = useState<string | null>(null);
  const [forkSuccessMsgId, setForkSuccessMsgId] = useState<string | null>(null);
  const [forkError, setForkError] = useState<string | null>(null);
  const canFork = source === "claude" && Boolean(filePath && projectPath);

  const handleFork = useCallback(
    async (userMsgId: string) => {
      if (!filePath || !projectPath) return;
      setForkError(null);
      setForkingMsgId(userMsgId);
      try {
        await api.forkAndResume(source, filePath, userMsgId, projectPath, terminalShell);
        setForkSuccessMsgId(userMsgId);
        setTimeout(() => setForkSuccessMsgId(null), 1800);
        void refreshInBackground();
      } catch (error) {
        setForkError(error instanceof Error ? error.message : String(error));
      } finally {
        setForkingMsgId(null);
      }
    },
    [filePath, projectPath, refreshInBackground, source, terminalShell],
  );

  if (items.length === 0) {
    return (
      <div className="mx-auto max-w-3xl px-6 py-10 text-center text-sm text-muted-foreground">
        当前会话还没有用户提问可供汇总。
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-3xl space-y-3 px-4 py-6 sm:px-6">
      <div className="mb-2 flex items-center justify-between text-xs text-muted-foreground">
        <span>
          共 {items.length} 条提问
          {isThreaded && "（按父子关系展示）"}
        </span>
        <span>
          {canFork ? '点击任意一条跳转 · “回复此处”从该消息分叉新会话' : "点击任意一条跳转"}
        </span>
      </div>

      {forkError && (
        <div className="mb-3 rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-400">
          分叉失败：{forkError}
          <button
            type="button"
            onClick={() => setForkError(null)}
            className="ml-2 opacity-70 hover:opacity-100"
          >
            ×
          </button>
        </div>
      )}

      {items.map((item, index) => {
        const isForking = forkingMsgId === item.messageId;
        const isForkSucceeded = forkSuccessMsgId === item.messageId;
        const indent = Math.min(item.depth, 4);

        return (
          <div
            key={item.messageId}
            className="relative"
            style={{ paddingLeft: `${indent * 1.25}rem` }}
          >
            {item.parentMessageId && (
              <span
                aria-hidden
                className="absolute top-0 h-full border-l border-dashed border-border"
                style={{ left: `${(indent - 1) * 1.25 + 0.5}rem` }}
              />
            )}
            <div className="rounded-lg border border-border bg-card transition-colors hover:border-primary/50 hover:bg-accent/60">
              <button
                type="button"
                onClick={() => onSelect(item.messageId)}
                className="group w-full px-4 py-3 text-left"
                title={item.question}
              >
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <span className="inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-muted px-1.5 font-mono text-[11px]">
                    {index + 1}
                  </span>
                  <MessageCircleQuestion className="h-3.5 w-3.5 text-emerald-600 dark:text-emerald-400" />
                  <span>用户提问</span>
                  {item.branchCount > 1 && (
                    <span
                      className="rounded bg-amber-500/15 px-1.5 py-0.5 font-mono text-[10px] text-amber-600 dark:text-amber-400"
                      title="此消息后存在多个分叉"
                    >
                      {source === "omp" ? `${item.branchCount} 条路径` : `${item.branchCount} 条分叉`}
                    </span>
                  )}
                  {source === "omp" && item.branchIndex > 0 && (
                    <span
                      className="rounded bg-fuchsia-500/15 px-1.5 py-0.5 font-mono text-[10px] text-fuchsia-600 dark:text-fuchsia-400"
                      title="此问题由回退到同一父节点后创建"
                    >
                      回退分支
                    </span>
                  )}
                </div>
                <p className="mt-1.5 line-clamp-3 whitespace-pre-wrap text-sm font-medium text-foreground">
                  {item.question}
                </p>
                <div className="mt-2 flex items-start gap-2 rounded-md bg-muted/30 px-3 py-2">
                  <CornerDownRight className="mt-0.5 h-3.5 w-3.5 shrink-0 text-sky-600 dark:text-sky-400" />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
                      <span>{assistantName} 回复</span>
                      {item.replyModel && (
                        <span className="rounded bg-background px-1.5 py-0.5 font-mono">
                          {item.replyModel}
                        </span>
                      )}
                      {item.replyTimestamp && <span>· {item.replyTimestamp}</span>}
                      {item.hasTool && (
                        <span className="rounded bg-amber-500/15 px-1.5 py-0.5 text-amber-600 dark:text-amber-400">
                          含工具调用
                        </span>
                      )}
                    </div>
                    <p className="mt-1 line-clamp-2 text-sm text-muted-foreground">
                      {item.replyPreview || "尚无可见回复"}
                    </p>
                  </div>
                </div>
              </button>
              {canFork && (
                <div className="border-t border-border px-4 py-2">
                  <button
                    type="button"
                    onClick={() => void handleFork(item.messageId)}
                    disabled={isForking}
                    className={`inline-flex items-center gap-1 rounded border px-1.5 py-0.5 font-mono text-[10px] transition-colors ${
                      isForkSucceeded
                        ? "border-emerald-500/50 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
                        : "border-border bg-background hover:border-primary hover:text-primary"
                    } ${isForking ? "opacity-60" : ""}`}
                    title="从此条消息分叉新会话并在终端打开"
                  >
                    {isForking ? (
                      <Loader2 className="h-3 w-3 animate-spin" />
                    ) : isForkSucceeded ? (
                      <Check className="h-3 w-3" />
                    ) : (
                      <GitBranch className="h-3 w-3" />
                    )}
                    {isForkSucceeded ? "已分叉" : "回复此处"}
                  </button>
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
});
