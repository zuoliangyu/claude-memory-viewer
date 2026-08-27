import { execFileSync } from "node:child_process";
import { existsSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const logo = resolve(root, "public/logo.png");
const marker = resolve(root, "src-tauri/icons/icon.png");

if (!existsSync(logo)) {
  console.log("[generate-icons] public/logo.png not found, skipping");
  process.exit(0);
}

let needsRegen = true;
if (existsSync(marker)) {
  needsRegen = statSync(logo).mtimeMs > statSync(marker).mtimeMs;
}

if (needsRegen) {
  console.log("[generate-icons] Logo changed, regenerating icons...");
  const npx = process.platform === "win32" ? "npx.cmd" : "npx";
  execFileSync(npx, ["tauri", "icon", logo], {
    cwd: root,
    stdio: "inherit",
  });
  console.log("[generate-icons] Done");
} else {
  console.log("[generate-icons] Icons up to date, skipping");
}
