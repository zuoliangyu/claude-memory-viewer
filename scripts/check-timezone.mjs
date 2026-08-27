import assert from "node:assert/strict";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { loadTypeScriptModule } from "./load-typescript-module.mjs";

process.env.TZ = "Asia/Shanghai";
const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const {
  formatDateOnly,
  formatDateTime,
  formatShortDateTime,
  normalizeTimeZone,
} = await loadTypeScriptModule(resolve(scriptDirectory, "../src/utils/dateTime.ts"));

const timestamp = "2026-06-26T13:41:38Z";
assert.equal(formatShortDateTime(timestamp), "06-26 21:41:38");
assert.equal(formatShortDateTime(timestamp, "Asia/Shanghai"), "06-26 21:41:38");
assert.equal(formatDateTime(timestamp, "UTC"), "2026-06-26 13:41:38");
assert.equal(formatDateOnly(timestamp, "Asia/Shanghai"), "2026-06-26");
assert.equal(normalizeTimeZone("Invalid/Zone"), "");
