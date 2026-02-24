import { execSync } from "child_process";
import { statSync, existsSync } from "fs";
import { resolve } from "path";

const logo = resolve("public/logo.png");
const marker = resolve("src-tauri/icons/icon.png");

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
  execSync("npx tauri icon public/logo.png", { stdio: "inherit" });
  console.log("[generate-icons] Done");
} else {
  console.log("[generate-icons] Icons up to date, skipping");
}
