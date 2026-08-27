import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

import ts from "typescript";

export async function loadTypeScriptModule(path) {
  const source = await readFile(path, "utf8");
  const { outputText, diagnostics } = ts.transpileModule(source, {
    fileName: path,
    reportDiagnostics: true,
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
    },
  });

  const errors = diagnostics?.filter(
    (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
  );
  if (errors?.length) {
    throw new Error(
      errors
        .map((diagnostic) =>
          ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n"),
        )
        .join("\n"),
    );
  }

  const sourceUrl = pathToFileURL(path).href;
  const withSourceUrl = outputText + "\n//# sourceURL=" + sourceUrl;
  const encoded = Buffer.from(withSourceUrl).toString("base64");
  return import("data:text/javascript;base64," + encoded);
}
