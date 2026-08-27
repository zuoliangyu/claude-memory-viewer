import { readFile, readdir, stat } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const root = resolve(scriptDirectory, "..");

function parseArguments(values) {
  const options = { path: null, ipcThresholdMs: 5000 };
  for (let index = 0; index < values.length; index += 1) {
    const argument = values[index];
    if (argument === "--path") {
      options.path = values[++index];
    } else if (argument === "--ipc-threshold-ms") {
      options.ipcThresholdMs = Number(values[++index]);
    } else {
      throw new Error("未知参数: " + argument);
    }
  }
  if (!Number.isFinite(options.ipcThresholdMs) || options.ipcThresholdMs < 0) {
    throw new Error("--ipc-threshold-ms 必须是非负数字");
  }
  return options;
}

async function findLatestLog() {
  const directory = resolve(root, "target", "perf");
  let names;
  try {
    names = await readdir(directory);
  } catch {
    return null;
  }

  const candidates = await Promise.all(
    names
      .filter((name) => /^dev-.*\.log$/.test(name))
      .map(async (name) => {
        const path = resolve(directory, name);
        return { path, modified: (await stat(path)).mtimeMs };
      }),
  );
  candidates.sort((left, right) => right.modified - left.modified);
  return candidates[0]?.path ?? null;
}

function slowest(events, name) {
  return events
    .filter((event) => event.name === name)
    .sort(
      (left, right) =>
        Number(right.durationMs ?? 0) - Number(left.durationMs ?? 0),
    )[0];
}

function formatDuration(value) {
  return Number(value ?? 0).toFixed(1);
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const path = options.path ? resolve(options.path) : await findLatestLog();
  if (!path) {
    throw new Error(
      "未找到 target/perf/dev-*.log，请先通过 menu 选择性能诊断开发模式。",
    );
  }

  const events = (await readFile(path, "utf8"))
    .split(/\r?\n/)
    .flatMap((line) => {
      const match = line.match(/^\[ASV-PERF\]\s+(\{.*\})$/);
      if (!match) return [];
      try {
        return [JSON.parse(match[1])];
      } catch {
        return [];
      }
    });

  const ipcEvents = events.filter(
    (event) => event.name === "messages.ipc_roundtrip",
  );
  if (ipcEvents.length === 0) {
    throw new Error("日志中没有 messages.ipc_roundtrip，尚未完成一次消息读取。");
  }

  const slowestIpc = slowest(events, "messages.ipc_roundtrip");
  const slowestRefresh = slowest(events, "background_refresh.completed");
  const slowestSessionCost = slowest(events, "stats.session_cost_backend");
  const longTaskCount = events.filter(
    (event) => event.name === "browser.long_task",
  ).length;

  console.log("日志: " + path);
  console.log(
    "最慢消息 IPC: " +
      formatDuration(slowestIpc.durationMs) +
      " ms（" +
      (slowestIpc.fields?.messages ?? 0) +
      " 条消息，约 " +
      (slowestIpc.fields?.approximateTextMb ?? 0) +
      " MB 文本）",
  );
  if (slowestRefresh) {
    console.log(
      "最慢后台刷新: " +
        formatDuration(slowestRefresh.durationMs) +
        " ms（reason=" +
        (slowestRefresh.fields?.reason ?? "") +
        ", forceReload=" +
        (slowestRefresh.fields?.forceReload ?? false) +
        "）",
    );
  }
  if (slowestSessionCost) {
    console.log(
      "最慢会话账单: " +
        formatDuration(slowestSessionCost.durationMs) +
        " ms（" +
        (slowestSessionCost.fields?.requests ?? 0) +
        " 次请求）",
    );
  }
  console.log("浏览器长任务: " + longTaskCount);

  if (Number(slowestIpc.durationMs) > options.ipcThresholdMs) {
    throw new Error(
      "消息 IPC 超过 " + options.ipcThresholdMs.toFixed(0) + " ms 阈值",
    );
  }
  console.log(
    "PASS: 消息 IPC 未超过 " +
      options.ipcThresholdMs.toFixed(0) +
      " ms 阈值",
  );
}

main().catch((error) => {
  console.error("FAIL: " + error.message);
  process.exitCode = 1;
});
