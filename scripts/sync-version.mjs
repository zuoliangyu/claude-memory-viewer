import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const mode = process.argv[2] ?? "sync";
if (mode !== "sync" && mode !== "check") {
  console.error("[sync-version] Unknown mode: " + mode);
  console.error("[sync-version] Expected: sync or check");
  process.exit(2);
}

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
const lockfilePath = resolve(root, "package-lock.json");

const tauriConf = JSON.parse(readFileSync(tauriConfPath, "utf-8"));

function readLockfileVersion() {
  if (!existsSync(lockfilePath)) return { topLevel: null, rootPkg: null };
  const lock = JSON.parse(readFileSync(lockfilePath, "utf-8"));
  return {
    topLevel: lock.version ?? null,
    rootPkg: lock.packages?.[""]?.version ?? null,
    raw: lock,
  };
}

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
  const lock = readLockfileVersion();
  if (lock.topLevel && lock.topLevel !== version) {
    console.error(
      `[sync-version] MISMATCH: package-lock.json (root) "${lock.topLevel}" != package.json "${version}"`,
    );
    mismatch = true;
  }
  if (lock.rootPkg && lock.rootPkg !== version) {
    console.error(
      `[sync-version] MISMATCH: package-lock.json (packages."") "${lock.rootPkg}" != package.json "${version}"`,
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

  // Sync the two version slots in package-lock.json (top-level and the
  // root package entry under packages[""]). Avoids running `npm install`
  // here so we don't perturb dependency resolution as a side effect.
  const lock = readLockfileVersion();
  if (lock.raw) {
    let lockChanged = false;
    if (lock.topLevel && lock.topLevel !== version) {
      lock.raw.version = version;
      lockChanged = true;
    }
    if (lock.rootPkg && lock.rootPkg !== version) {
      lock.raw.packages[""].version = version;
      lockChanged = true;
    }
    if (lockChanged) {
      writeFileSync(lockfilePath, JSON.stringify(lock.raw, null, 2) + "\n");
      console.log(`[sync-version] package-lock.json -> ${version}`);
      changed = true;
    }
  }

  if (!changed) {
    console.log(`[sync-version] Already in sync: ${version}`);
  }
}
