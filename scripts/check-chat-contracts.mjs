import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(scriptDirectory, "..");

async function readSource(relativePath) {
  return readFile(resolve(repositoryRoot, relativePath), "utf8");
}

const [webChat, desktopChat, ompProvider, messagesPage] = await Promise.all([
  readSource("crates/session-web/src/chat_ws.rs"),
  readSource("src-tauri/src/commands/chat.rs"),
  readSource("crates/session-core/src/provider/omp.rs"),
  readSource("src/components/message/MessagesPage.tsx"),
]);

const webCliProcess = webChat.slice(
  webChat.indexOf("async fn run_cli_process("),
  webChat.indexOf("/// Run a Codex chat through the app-server"),
);
assert.match(
  webCliProcess,
  /cmd\.arg\("-p"\)\.arg\(prompt\);/,
  "Web Claude/OMP chat must pass the prompt to print mode",
);

assert.match(
  desktopChat,
  /if source != "omp" \{\s*cmd\.env_clear\(\);/,
  "Desktop OMP chat must inherit its profile, XDG, and provider environment",
);
assert.match(
  desktopChat,
  /if source == "omp" \{\s*return;\s*\}\s*for key in &\[/,
  "Desktop OMP chat must not remove provider credentials from its environment",
);

assert.match(
  ompProvider,
  /XDG_DATA_HOME/,
  "OMP session discovery must support XDG data migration",
);

assert.doesNotMatch(
  messagesPage,
  /const cliAvailable = source === "omp" \|\|/,
  "OMP availability must come from CLI detection",
);
