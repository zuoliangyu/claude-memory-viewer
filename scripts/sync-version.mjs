import { readFileSync, writeFileSync } from "fs";
import { resolve } from "path";

const root = resolve(import.meta.dirname, "..");
const mode = process.argv[2]; // "sync" or "check"

// 1. Read version from package.json (single source of truth)
const pkg = JSON.parse(readFileSync(resolve(root, "package.json"), "utf-8"));
const version = pkg.version;

// 2. Read other files
const tauriConfPath = resolve(root, "src-tauri/tauri.conf.json");
const cargoTomlPath = resolve(root, "src-tauri/Cargo.toml");

const tauriConf = JSON.parse(readFileSync(tauriConfPath, "utf-8"));
const cargoToml = readFileSync(cargoTomlPath, "utf-8");
const cargoMatch = cargoToml.match(/^version\s*=\s*"(.+?)"/m);
const cargoVersion = cargoMatch?.[1];

if (mode === "check") {
  // Verify all versions match
  let mismatch = false;
  if (tauriConf.version !== version) {
    console.error(
      `[sync-version] MISMATCH: tauri.conf.json "${tauriConf.version}" != package.json "${version}"`,
    );
    mismatch = true;
  }
  if (cargoVersion !== version) {
    console.error(
      `[sync-version] MISMATCH: Cargo.toml "${cargoVersion}" != package.json "${version}"`,
    );
    mismatch = true;
  }
  if (mismatch) {
    console.error(
      '[sync-version] Run "npm run sync-version" to fix, then rebuild.',
    );
    process.exit(1);
  }
  console.log(`[sync-version] All versions consistent: ${version}`);
} else {
  // Sync mode (default): write package.json version to other files
  let changed = false;

  if (tauriConf.version !== version) {
    tauriConf.version = version;
    writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + "\n");
    console.log(`[sync-version] tauri.conf.json -> ${version}`);
    changed = true;
  }

  if (cargoVersion !== version) {
    const updated = cargoToml.replace(
      /^(version\s*=\s*")(.+?)(")/m,
      `$1${version}$3`,
    );
    writeFileSync(cargoTomlPath, updated);
    console.log(`[sync-version] Cargo.toml -> ${version}`);
    changed = true;
  }

  if (!changed) {
    console.log(`[sync-version] Already in sync: ${version}`);
  }
}
