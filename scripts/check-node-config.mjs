import assert from "node:assert/strict";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { loadTypeScriptModule } from "./load-typescript-module.mjs";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const { normalizeNodeUrl } = await loadTypeScriptModule(
  resolve(scriptDirectory, "../src/services/nodeConfig.ts"),
);

assert.equal(normalizeNodeUrl(" https://viewer.example.com/ "), "https://viewer.example.com");
assert.equal(normalizeNodeUrl("http://192.168.1.20:3000/"), "http://192.168.1.20:3000");
assert.throws(() => normalizeNodeUrl("ftp://viewer.example.com"));
assert.throws(() => normalizeNodeUrl("https://user:pass@viewer.example.com"));
assert.throws(() => normalizeNodeUrl("https://viewer.example.com/subpath"));
