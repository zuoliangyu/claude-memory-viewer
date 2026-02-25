import { ChevronDown, Bot, Terminal } from "lucide-react";

interface Props {
  source: "claude" | "codex";
  model: string;
  onClick: () => void;
}

/** Derive a short display name from a full model ID. */
function shortName(id: string): string {
  // Remove date suffixes like -20250514
  let name = id.replace(/-\d{8}$/, "");
  // Remove common prefixes
  name = name.replace(/^claude-/, "");
  name = name.replace(/^codex-/, "codex ");
  // Capitalize segments
  return name
    .split("-")
    .map((s) => s.charAt(0).toUpperCase() + s.slice(1))
    .join(" ");
}

export function FloatingModelBadge({ source, model, onClick }: Props) {
  const colorClass =
    source === "codex"
      ? "border-green-500/30 bg-green-500/10 text-green-400 hover:bg-green-500/20"
      : "border-orange-500/30 bg-orange-500/10 text-orange-400 hover:bg-orange-500/20";
  const Icon = source === "codex" ? Terminal : Bot;

  return (
    <div className="sticky top-0 z-10 flex justify-center py-1.5 pointer-events-none">
      <button
        onClick={onClick}
        className={`pointer-events-auto inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full border text-xs font-medium backdrop-blur-sm transition-colors ${colorClass}`}
      >
        <Icon className="w-3 h-3" />
        <span>{model ? shortName(model) : "选择模型"}</span>
        <ChevronDown className="w-3 h-3 opacity-60" />
      </button>
    </div>
  );
}
