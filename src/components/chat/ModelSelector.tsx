import { useEffect, useRef, useState, useMemo } from "react";
import { useChatStore } from "../../stores/chatStore";
import {
  Search,
  Check,
  Loader2,
  AlertCircle,
  X,
  RefreshCw,
  Plus,
  Trash2,
} from "lucide-react";
import type { ModelInfo } from "../../types/chat";

interface Props {
  open: boolean;
  onClose: () => void;
  onSelect: (modelId: string) => void;
}

export function ModelSelector({ open, onClose, onSelect }: Props) {
  const {
    source,
    model,
    modelList,
    modelListLoading,
    modelListError,
    fetchModelList,
    addCustomModel,
    removeCustomModel,
  } = useChatStore();

  const [search, setSearch] = useState("");
  const [highlightIndex, setHighlightIndex] = useState(0);
  const [showAddInput, setShowAddInput] = useState(false);
  const [newModelId, setNewModelId] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const addInputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Fetch model list when opened
  useEffect(() => {
    if (open) {
      fetchModelList(source);
      setSearch("");
      setHighlightIndex(0);
      setShowAddInput(false);
      setNewModelId("");
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open, source, fetchModelList]);

  // Focus add input when shown
  useEffect(() => {
    if (showAddInput) {
      setTimeout(() => addInputRef.current?.focus(), 50);
    }
  }, [showAddInput]);

  // Filter models by search
  const filtered = useMemo(() => {
    if (!search.trim()) return modelList;
    const q = search.toLowerCase();
    return modelList.filter(
      (m) =>
        m.id.toLowerCase().includes(q) ||
        m.name.toLowerCase().includes(q) ||
        m.group.toLowerCase().includes(q)
    );
  }, [modelList, search]);

  // Whether the search term could be used as a custom model ID
  const canUseSearchAsCustom = search.trim().length > 0 && filtered.length === 0 && !showAddInput;

  // Group filtered models
  const grouped = useMemo(() => {
    const groups: Record<string, ModelInfo[]> = {};
    for (const m of filtered) {
      if (!groups[m.group]) groups[m.group] = [];
      groups[m.group].push(m);
    }
    return groups;
  }, [filtered]);

  // Flat list for keyboard navigation
  const flatList = useMemo(() => filtered, [filtered]);

  // Get custom model IDs for delete detection
  const customModelIds = useMemo(() => {
    const key = `chat_customModels_${source}`;
    try {
      return new Set<string>(JSON.parse(localStorage.getItem(key) || "[]"));
    } catch {
      return new Set<string>();
    }
  }, [source, modelList]); // eslint-disable-line react-hooks/exhaustive-deps

  // Reset highlight when search changes
  useEffect(() => {
    setHighlightIndex(0);
  }, [search]);

  // Scroll highlighted item into view
  useEffect(() => {
    const el = listRef.current?.querySelector(
      `[data-index="${highlightIndex}"]`
    );
    el?.scrollIntoView({ block: "nearest" });
  }, [highlightIndex]);

  const handleUseSearchAsCustom = () => {
    const id = search.trim();
    if (id) {
      addCustomModel(id);
      onSelect(id);
      onClose();
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (showAddInput) return; // let add input handle keys
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setHighlightIndex((i) => Math.min(i + 1, flatList.length - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setHighlightIndex((i) => Math.max(i - 1, 0));
        break;
      case "Enter":
        e.preventDefault();
        if (canUseSearchAsCustom) {
          handleUseSearchAsCustom();
        } else if (flatList[highlightIndex]) {
          onSelect(flatList[highlightIndex].id);
          onClose();
        }
        break;
      case "Escape":
        e.preventDefault();
        onClose();
        break;
    }
  };

  const handleAddModel = () => {
    const id = newModelId.trim();
    if (!id) return;
    addCustomModel(id);
    onSelect(id);
    setNewModelId("");
    setShowAddInput(false);
  };

  if (!open) return null;

  let flatIndex = -1;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh] bg-black/50"
      onClick={onClose}
    >
      <div
        className="bg-card border border-border rounded-lg shadow-2xl w-[28rem] max-w-[90vw] max-h-[60vh] flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        {/* Search input */}
        <div className="flex items-center gap-2 px-3 py-2.5 border-b border-border">
          <Search className="w-4 h-4 text-muted-foreground shrink-0" />
          <input
            ref={inputRef}
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索或输入模型 ID (如 opus, claude-sonnet-4-6)"
            className="flex-1 bg-transparent text-sm text-foreground placeholder:text-muted-foreground outline-none"
          />
          <button
            onClick={onClose}
            className="p-0.5 rounded text-muted-foreground hover:text-foreground transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Management bar */}
        <div className="flex items-center gap-1.5 px-3 py-1.5 border-b border-border bg-muted/50">
          <button
            onClick={() => fetchModelList(source)}
            disabled={modelListLoading}
            className="flex items-center gap-1 px-2 py-0.5 text-xs rounded border border-border bg-card text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors disabled:opacity-50"
            title="刷新模型列表"
          >
            <RefreshCw
              className={`w-3 h-3 ${modelListLoading ? "animate-spin" : ""}`}
            />
            刷新
          </button>
          <button
            onClick={() => setShowAddInput((v) => !v)}
            className={`flex items-center gap-1 px-2 py-0.5 text-xs rounded border transition-colors ${
              showAddInput
                ? "border-primary bg-primary/10 text-primary"
                : "border-border bg-card text-muted-foreground hover:text-foreground hover:bg-accent/50"
            }`}
            title="手动添加模型 ID"
          >
            <Plus className="w-3 h-3" />
            添加
          </button>
          <span className="flex-1" />
          <span className="text-[10px] text-muted-foreground">
            {modelList.length} 个模型
          </span>
        </div>

        {/* Add custom model input */}
        {showAddInput && (
          <div className="flex items-center gap-2 px-3 py-2 border-b border-border bg-muted/30">
            <input
              ref={addInputRef}
              type="text"
              value={newModelId}
              onChange={(e) => setNewModelId(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  handleAddModel();
                }
                if (e.key === "Escape") {
                  e.preventDefault();
                  setShowAddInput(false);
                }
                e.stopPropagation();
              }}
              placeholder="输入模型 ID，如 opus 或 claude-sonnet-4-6"
              className="flex-1 bg-muted border border-border rounded px-2 py-1 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
            />
            <button
              onClick={handleAddModel}
              disabled={!newModelId.trim()}
              className="px-2 py-1 text-xs rounded bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
            >
              添加并选择
            </button>
          </div>
        )}

        {/* Model list */}
        <div ref={listRef} className="flex-1 overflow-y-auto py-1">
          {modelListLoading ? (
            <div className="flex items-center justify-center gap-2 py-8 text-sm text-muted-foreground">
              <Loader2 className="w-4 h-4 animate-spin" />
              加载模型列表...
            </div>
          ) : modelListError ? (
            <div className="flex flex-col items-center gap-2 py-6 text-sm text-red-400">
              <div className="flex items-center gap-2">
                <AlertCircle className="w-4 h-4" />
                {modelListError}
              </div>
              <button
                onClick={() => fetchModelList(source)}
                className="text-xs text-muted-foreground hover:text-foreground transition-colors"
              >
                点击重试
              </button>
            </div>
          ) : flatList.length === 0 ? (
            <div className="flex flex-col items-center gap-3 py-6">
              {canUseSearchAsCustom ? (
                <button
                  onClick={handleUseSearchAsCustom}
                  className="flex items-center gap-2 px-4 py-2 text-sm rounded-lg border border-primary/50 bg-primary/10 text-foreground hover:bg-primary/20 transition-colors"
                >
                  <span>使用</span>
                  <code className="px-1.5 py-0.5 rounded bg-muted text-xs font-mono">{search.trim()}</code>
                  <span>作为模型 ID</span>
                </button>
              ) : (
                <span className="text-sm text-muted-foreground">
                  {search ? "未找到匹配模型" : "无可用模型"}
                </span>
              )}
            </div>
          ) : (
            Object.entries(grouped).map(([group, models]) => (
              <div key={group}>
                <div className="px-3 py-1 text-xs font-medium text-muted-foreground uppercase tracking-wider">
                  {group}
                </div>
                {models.map((m) => {
                  flatIndex++;
                  const idx = flatIndex;
                  const isSelected = m.id === model;
                  const isHighlighted = idx === highlightIndex;
                  const isCustom = customModelIds.has(m.id);
                  return (
                    <div
                      key={m.id}
                      data-index={idx}
                      className={`flex items-center gap-2 px-3 py-1.5 text-sm transition-colors group ${
                        isHighlighted
                          ? "bg-accent text-accent-foreground"
                          : "text-foreground hover:bg-accent/50"
                      }`}
                      onMouseEnter={() => setHighlightIndex(idx)}
                    >
                      <button
                        className="flex-1 flex items-center gap-2 text-left min-w-0"
                        onClick={() => {
                          onSelect(m.id);
                          onClose();
                        }}
                      >
                        <span className="truncate flex-1">{m.name}</span>
                        <span className="text-xs text-muted-foreground truncate max-w-[10rem]">
                          {m.id !== m.name ? m.id : ""}
                        </span>
                        {isSelected && (
                          <Check className="w-3.5 h-3.5 text-primary shrink-0" />
                        )}
                      </button>
                      {isCustom && (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            removeCustomModel(m.id);
                          }}
                          className="p-0.5 rounded text-muted-foreground/0 group-hover:text-muted-foreground hover:!text-red-400 transition-colors shrink-0"
                          title="移除自定义模型"
                        >
                          <Trash2 className="w-3 h-3" />
                        </button>
                      )}
                    </div>
                  );
                })}
              </div>
            ))
          )}
        </div>

        {/* Footer hint */}
        <div className="px-3 py-1.5 border-t border-border text-xs text-muted-foreground flex items-center gap-3">
          <span>
            <kbd className="px-1 py-0.5 rounded bg-muted text-[10px]">
              ↑↓
            </kbd>{" "}
            导航
          </span>
          <span>
            <kbd className="px-1 py-0.5 rounded bg-muted text-[10px]">
              Enter
            </kbd>{" "}
            选择
          </span>
          <span>
            <kbd className="px-1 py-0.5 rounded bg-muted text-[10px]">
              Esc
            </kbd>{" "}
            关闭
          </span>
          <span className="flex-1" />
          <span>输入任意 ID 可直接使用</span>
        </div>
      </div>
    </div>
  );
}
