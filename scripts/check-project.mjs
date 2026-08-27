const checks = [
  ["节点配置", "./check-node-config.mjs"],
  ["时区格式化", "./check-timezone.mjs"],
];

for (const [label, modulePath] of checks) {
  try {
    await import(modulePath);
    console.log("[check-project] PASS: " + label);
  } catch (error) {
    console.error("[check-project] FAIL: " + label);
    console.error(error);
    process.exitCode = 1;
    break;
  }
}
