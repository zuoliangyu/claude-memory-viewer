import { readFileSync, writeFileSync, existsSync } from "fs";
import { resolve } from "path";

const root = resolve(import.meta.dirname, "..");
const mode = process.argv[2]; // "sync" or "check"

// 1. Read version from package.json (single source of truth)
const pkg = JSON.parse(readFileSync(resolve(root, "package.json"), "utf-8"));
const version = pkg.version;

// 2. Read other files
const tauriConfPath = resolve(root, "src-tauri/tauri.conf.json");
const cargoTomlPaths = [
  resolve(root, "src-tauri/Cargo.toml"),
  resolve(root, "crates/session-core/Cargo.toml"),
  resolve(root, "crates/session-web/Cargo.toml"),
];

const tauriConf = JSON.parse(readFileSync(tauriConfPath, "utf-8"));

function readCargoVersion(path) {
  if (!existsSync(path)) return null;
  const content = readFileSync(path, "utf-8");
  const match = content.match(/^version\s*=\s*"(.+?)"/m);
  return match?.[1] ?? null;
}

if (mode === "check") {
  // Verify all versions match
  let mismatch = false;
  if (tauriConf.version !== version) {
    console.error(
      `[sync-version] MISMATCH: tauri.conf.json "${tauriConf.version}" != package.json "${version}"`,
    );
    mismatch = true;
  }
  for (const cargoPath of cargoTomlPaths) {
    const cargoVersion = readCargoVersion(cargoPath);
    if (cargoVersion && cargoVersion !== version) {
      const rel = cargoPath.replace(root + "/", "").replace(root + "\\", "");
      console.error(
        `[sync-version] MISMATCH: ${rel} "${cargoVersion}" != package.json "${version}"`,
      );
      mismatch = true;
    }
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

  for (const cargoPath of cargoTomlPaths) {
    if (!existsSync(cargoPath)) continue;
    const cargoVersion = readCargoVersion(cargoPath);
    if (cargoVersion && cargoVersion !== version) {
      const content = readFileSync(cargoPath, "utf-8");
      const updated = content.replace(
        /^(version\s*=\s*")(.+?)(")/m,
        `$1${version}$3`,
      );
      writeFileSync(cargoPath, updated);
      const rel = cargoPath.replace(root + "/", "").replace(root + "\\", "");
      console.log(`[sync-version] ${rel} -> ${version}`);
      changed = true;
    }
  }

  if (!changed) {
    console.log(`[sync-version] Already in sync: ${version}`);
  }
}
