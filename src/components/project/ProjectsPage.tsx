import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useAppStore } from "../../stores/appStore";
import type { ProjectEntry } from "../../types";
import {
  FolderOpen,
  FolderClock,
  Clock,
  Hash,
  Tag,
  MoreHorizontal,
  AlertCircle,
  CheckSquare,
  X,
  Trash2,
  Loader2,
  RefreshCw,
} from "lucide-react";
import { formatDistanceToNow } from "date-fns";
import { zhCN } from "date-fns/locale";
import { ProjectActionsMenu } from "./ProjectActionsMenu";
import { DeleteProjectDialog } from "./DeleteProjectDialog";
import { ScanProgressView } from "../common/ScanProgressView";
import { collapseDirectBuckets, DIRECT_GROUP_ID } from "../../utils/directChat";

export function ProjectsPage() {
  const navigate = useNavigate();
  const {
    source,
    projects,
    loadProjects,
    projectsLoading,
    refreshInBackground,
    crossProjectTags,
    globalTagFilter,
    loadCrossProjectTags,
    setGlobalTagFilter,
    deleteProject,
    setProjectAlias,
  } = useAppStore();

  // ⋯ 操作菜单状态
  const [actionsMenu, setActionsMenu] = useState<{
    project: ProjectEntry;
    anchorRect: DOMRect;
  } | null>(null);

  // 删除确认对话框状态
  const [deleteTarget, setDeleteTarget] = useState<ProjectEntry | null>(null);

  // 重命名（别名）对话框状态
  const [renameTarget, setRenameTarget] = useState<ProjectEntry | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [renameError, setRenameError] = useState<string | null>(null);
  const [renameLoading, setRenameLoading] = useState(false);

  // 多选批量删除
  const [selectMode, setSelectMode] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set()); // by project id
  const [batchBusy, setBatchBusy] = useState(false);
  const [batchDeleteOpen, setBatchDeleteOpen] = useState(false);
  const [batchWithCcConfig, setBatchWithCcConfig] = useState(false);
  const [refreshing, setRefreshing] = useState(false);

  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      await refreshInBackground(true, true);
    } finally {
      setRefreshing(false);
    }
  };

  const toggleSelected = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const exitSelectMode = () => {
    setSelectMode(false);
    setSelected(new Set());
  };

  useEffect(() => {
    loadProjects();
    // Cross-project tags only feed the optional tag-filter chips and aren't
    // needed for the first paint of the project list — defer off the critical
    // path so the list shows as soon as projects resolve.
    const t = setTimeout(() => loadCrossProjectTags(), 0);
    return () => clearTimeout(t);
  }, [source]);

  // Deduplicated sorted list of all tags across projects
  const allGlobalTags = useMemo(() => {
    const tagSet = new Set<string>();
    for (const tags of Object.values(crossProjectTags)) {
      for (const tag of tags) {
        tagSet.add(tag);
      }
    }
    return Array.from(tagSet).sort();
  }, [crossProjectTags]);

  const toggleGlobalTag = (tag: string) => {
    if (globalTagFilter.includes(tag)) {
      setGlobalTagFilter(globalTagFilter.filter((t) => t !== tag));
    } else {
      setGlobalTagFilter([...globalTagFilter, tag]);
    }
  };

  // Filter projects by global tag filter
  const tagFilteredProjects =
    globalTagFilter.length > 0
      ? projects.filter((p) => {
          const projectTags = crossProjectTags[p.id] || [];
          return globalTagFilter.every((t) => projectTags.includes(t));
        })
      : projects;

  // Collapse Codex Desktop direct-chat date buckets into one pinned aggregate
  // card. Skip while a tag filter is active (the synthetic group carries no
  // tags, and per-date buckets shouldn't surface tag-matched anyway).
  const filteredProjects =
    globalTagFilter.length > 0
      ? tagFilteredProjects
      : collapseDirectBuckets(tagFilteredProjects);

  const emptyText =
    source === "claude"
      ? "未找到任何 Claude 项目。请确认 ~/.claude/projects/ 目录存在。"
      : source === "codex"
        ? "未找到任何 Codex 项目。请确认 ~/.codex/sessions/ 目录存在。"
        : source === "grok"
          ? "未找到任何 Grok 项目。请确认 ~/.grok/sessions/ 目录存在。"
          : "未找到任何 Oh My Pi 项目。请确认 ~/.omp/agent/sessions/ 目录存在。";

  const selectedProjects = filteredProjects.filter((p) => selected.has(p.id));

  // 网格虚拟化：按容器宽度算列数（1~3），把项目分块成「行」，只渲染可见行。
  const scrollRef = useRef<HTMLDivElement>(null);
  const [cols, setCols] = useState(3);
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const update = () => {
      const w = el.clientWidth;
      setCols(Math.min(3, Math.max(1, Math.floor(w / 300))));
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const rows = useMemo(() => {
    const out: ProjectEntry[][] = [];
    for (let i = 0; i < filteredProjects.length; i += cols) {
      out.push(filteredProjects.slice(i, i + cols));
    }
    return out;
  }, [filteredProjects, cols]);

  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 168,
    overscan: 4,
  });

  // 预计算 lastModified 相对时间，避免切换选择模式（整列表重渲染）时重跑 date-fns。
  // 用 filteredProjects 而非 projects，以覆盖合成的「直连对话」聚合卡。
  const lastModifiedMap = useMemo(() => {
    const m = new Map<string, string>();
    for (const p of filteredProjects) {
      if (p.lastModified) {
        m.set(p.id, formatDistanceToNow(new Date(p.lastModified), { addSuffix: true, locale: zhCN }));
      }
    }
    return m;
  }, [filteredProjects]);

  const handleBatchDelete = async () => {
    if (selectedProjects.length === 0) return;
    setBatchBusy(true);
    const level =
      source === "claude" && batchWithCcConfig ? "withCcConfig" : "sessionOnly";
    try {
      await Promise.all(selectedProjects.map((p) => deleteProject(p.id, level)));
      exitSelectMode();
    } catch (err) {
      console.error("Failed to batch delete projects:", err);
    } finally {
      setBatchBusy(false);
      setBatchDeleteOpen(false);
      setBatchWithCcConfig(false);
    }
  };

  return (
    <>
    <div className="flex flex-col h-full">
      <div className="px-6 pt-6 shrink-0">
      <div className="flex items-center mb-6">
        <h1 className="text-2xl font-bold">所有项目</h1>
        <div className="ml-auto flex items-center gap-2">
          <button
            onClick={handleRefresh}
            disabled={refreshing || projectsLoading || selectMode}
            className="text-xs px-3 py-1.5 rounded-md border border-border text-muted-foreground hover:text-foreground hover:border-primary/50 transition-colors flex items-center gap-1.5 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${refreshing ? "animate-spin" : ""}`} />
            刷新缓存
          </button>
          {projects.length > 0 &&
            (selectMode ? (
              <button
                onClick={exitSelectMode}
                className="text-xs px-3 py-1.5 rounded-md border border-border text-muted-foreground hover:text-foreground transition-colors flex items-center gap-1.5"
              >
                <X className="w-3.5 h-3.5" />
                退出选择
              </button>
            ) : (
              <button
                onClick={() => setSelectMode(true)}
                className="text-xs px-3 py-1.5 rounded-md border border-border text-muted-foreground hover:text-foreground hover:border-primary/50 transition-colors flex items-center gap-1.5"
              >
                <CheckSquare className="w-3.5 h-3.5" />
                选择
              </button>
            ))}
        </div>
      </div>

      {/* Global tag filter bar */}
      {allGlobalTags.length > 0 && (
        <div className="flex items-center gap-2 mb-4 flex-wrap">
          <Tag className="w-3.5 h-3.5 text-muted-foreground shrink-0" />
          {allGlobalTags.map((tag) => (
            <button
              key={tag}
              onClick={() => toggleGlobalTag(tag)}
              className={`px-2.5 py-1 text-xs rounded-full border transition-colors ${
                globalTagFilter.includes(tag)
                  ? "bg-primary text-primary-foreground border-primary"
                  : "bg-muted/50 text-muted-foreground border-border hover:border-primary/50"
              }`}
            >
              {tag}
            </button>
          ))}
          {globalTagFilter.length > 0 && (
            <button
              onClick={() => setGlobalTagFilter([])}
              className="px-2 py-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
            >
              清除筛选
            </button>
          )}
        </div>
      )}

      </div>

      {/* 项目网格（行分块虚拟化滚动容器） */}
      <div ref={scrollRef} className="flex-1 min-h-0 overflow-auto px-6 pt-2 pb-24">
      {projectsLoading ? (
        <ScanProgressView label="加载项目列表" />
      ) : filteredProjects.length === 0 ? (
        <div className="text-muted-foreground">
          {globalTagFilter.length > 0
            ? "没有匹配筛选条件的项目。"
            : emptyText}
        </div>
      ) : (
        <div style={{ height: rowVirtualizer.getTotalSize(), position: "relative", width: "100%" }}>
          {rowVirtualizer.getVirtualItems().map((virtualRow) => (
            <div
              key={virtualRow.index}
              data-index={virtualRow.index}
              ref={rowVirtualizer.measureElement}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${virtualRow.start}px)`,
                paddingBottom: "1rem",
              }}
            >
              <div className="grid gap-4" style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}>
                {rows[virtualRow.index].map((project) => (
            <div
              key={project.id}
              role="button"
              tabIndex={0}
              onClick={() => {
                if (selectMode) {
                  toggleSelected(project.id);
                  return;
                }
                if (project.id === DIRECT_GROUP_ID) {
                  navigate("/direct-chat");
                  return;
                }
                navigate(`/projects/${encodeURIComponent(project.id)}`);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  if (selectMode) {
                    toggleSelected(project.id);
                  } else if (project.id === DIRECT_GROUP_ID) {
                    navigate("/direct-chat");
                  } else {
                    navigate(`/projects/${encodeURIComponent(project.id)}`);
                  }
                }
              }}
              className={`relative bg-card border rounded-lg p-4 text-left hover:border-primary/50 hover:bg-accent/30 transition-all group cursor-pointer ${
                selected.has(project.id) ? "border-primary bg-primary/5" : "border-border"
              }`}
            >
              {/* 多选 checkbox（CSS 显隐，避免切换模式时整列表挂卸） */}
              <input
                type="checkbox"
                checked={selected.has(project.id)}
                onChange={() => toggleSelected(project.id)}
                onClick={(e) => e.stopPropagation()}
                className={`absolute top-2 right-2 accent-primary w-4 h-4 ${
                  selectMode && project.id !== DIRECT_GROUP_ID ? "" : "hidden"
                }`}
              />
              {/* ⋯ 操作按钮（聚合卡不显示——它不是真项目，不能删/改名） */}
              {project.id !== DIRECT_GROUP_ID && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    setActionsMenu({ project, anchorRect: e.currentTarget.getBoundingClientRect() });
                  }}
                  className={`absolute top-2 right-2 p-1.5 rounded transition-opacity text-muted-foreground hover:bg-accent/50 ${
                    selectMode ? "hidden" : "opacity-0 group-hover:opacity-100"
                  }`}
                  title="操作"
                >
                  <MoreHorizontal className="w-3.5 h-3.5" />
                </button>
              )}
              <div className="flex items-start gap-3">
                {project.isVirtual ? (
                  <FolderClock className="w-5 h-5 text-muted-foreground mt-0.5 shrink-0" />
                ) : (
                  <FolderOpen className="w-5 h-5 text-primary mt-0.5 shrink-0" />
                )}
                <div className="min-w-0 flex-1">
                  <h3 className="font-medium text-foreground truncate">
                    {project.alias ?? project.shortName}
                  </h3>
                  {project.alias && (
                    <p className="text-[10px] text-muted-foreground/60 truncate">
                      {project.shortName}
                    </p>
                  )}
                  <p
                    className={`text-xs truncate mt-1 ${
                      project.isVirtual
                        ? "text-muted-foreground italic"
                        : project.pathExists === false
                          ? "text-yellow-500"
                          : "text-muted-foreground"
                    }`}
                    title={
                      project.isVirtual
                        ? `${project.displayPath}（按日期合成的虚拟项目）`
                        : project.displayPath + (project.pathExists === false ? " (路径不存在，解码可能不准确)" : "")
                    }
                  >
                    {project.displayPath}
                    {!project.isVirtual && project.pathExists === false && (
                      <AlertCircle className="w-3 h-3 inline ml-1 -mt-0.5" />
                    )}
                  </p>
                  {/* Project tags */}
                  {crossProjectTags[project.id] && crossProjectTags[project.id].length > 0 && (
                    <div className="flex items-center gap-1.5 mt-2 flex-wrap">
                      {crossProjectTags[project.id].map((tag) => (
                        <span
                          key={tag}
                          className="inline-block px-2 py-0.5 text-xs rounded-full bg-primary/15 text-primary"
                        >
                          {tag}
                        </span>
                      ))}
                    </div>
                  )}
                  <div className="flex items-center gap-4 mt-3 text-xs text-muted-foreground">
                    <span className="flex items-center gap-1">
                      <Hash className="w-3 h-3" />
                      {project.sessionCount} 个会话
                    </span>
                    {project.lastModified && (
                      <span className="flex items-center gap-1">
                        <Clock className="w-3 h-3" />
                        {lastModifiedMap.get(project.id)}
                      </span>
                    )}
                  </div>
                  {project.modelProvider && (
                    <span className="mt-2 inline-block text-xs px-2 py-0.5 bg-muted rounded">
                      {project.modelProvider}
                    </span>
                  )}
                </div>
              </div>
            </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
      </div>
    </div>

    {/* ⋯ 操作菜单（portal） */}
    {actionsMenu && (
      <ProjectActionsMenu
        project={actionsMenu.project}
        source={source}
        anchorRect={actionsMenu.anchorRect}
        onClose={() => setActionsMenu(null)}
        onRename={(p) => {
          setRenameTarget(p);
          setRenameValue(p.alias ?? "");
          setRenameError(null);
        }}
        onDelete={(p) => { setDeleteTarget(p); }}
      />
    )}

    {/* 删除确认对话框 */}
    {deleteTarget && (
      <DeleteProjectDialog
        project={deleteTarget}
        source={source}
        onConfirm={async (level) => {
          await deleteProject(deleteTarget.id, level);
          setDeleteTarget(null);
          navigate("/projects");
        }}
        onCancel={() => { setDeleteTarget(null); }}
      />
    )}

    {/* 别名重命名对话框 */}
    {renameTarget && (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
        <div className="bg-card border border-border rounded-lg p-6 max-w-sm w-full mx-4 shadow-lg">
          <h3 className="text-lg font-semibold mb-1">设置工程别名</h3>
          <p className="text-xs text-muted-foreground mb-3">
            别名仅影响显示名称，不修改磁盘目录
          </p>
          <input
            type="text"
            value={renameValue}
            onChange={(e) => setRenameValue(e.target.value)}
            placeholder={renameTarget.shortName}
            autoFocus
            className="w-full bg-muted border border-border rounded px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setRenameTarget(null);
                setRenameError(null);
              }
            }}
          />
          {renameError && (
            <p className="text-xs text-red-400 mt-1">{renameError}</p>
          )}
          <div className="flex justify-between items-center mt-4">
            <div>
              {renameTarget.alias && (
                <button
                  onClick={async () => {
                    setRenameLoading(true);
                    try {
                      await setProjectAlias(renameTarget.id, null);
                      setRenameTarget(null);
                      setRenameError(null);
                    } catch (e) {
                      setRenameError(e instanceof Error ? e.message : String(e));
                    } finally {
                      setRenameLoading(false);
                    }
                  }}
                  disabled={renameLoading}
                  className="px-3 py-2 text-sm text-muted-foreground hover:text-foreground transition-colors"
                >
                  清除别名
                </button>
              )}
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => { setRenameTarget(null); setRenameError(null); }}
                disabled={renameLoading}
                className="px-4 py-2 text-sm rounded-md border border-border hover:bg-accent transition-colors"
              >
                取消
              </button>
              <button
                onClick={async () => {
                  setRenameLoading(true);
                  setRenameError(null);
                  try {
                    await setProjectAlias(renameTarget.id, renameValue.trim() || null);
                    setRenameTarget(null);
                  } catch (e) {
                    setRenameError(e instanceof Error ? e.message : String(e));
                  } finally {
                    setRenameLoading(false);
                  }
                }}
                disabled={renameLoading}
                className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
              >
                {renameLoading ? "保存中..." : "确认"}
              </button>
            </div>
          </div>
        </div>
      </div>
    )}

    {/* 多选底部浮动操作条 */}
    {selectMode && (
      <div className="fixed bottom-4 left-1/2 -translate-x-1/2 z-40 flex items-center gap-3 bg-card border border-border rounded-xl shadow-lg px-4 py-2.5">
        <button
          onClick={() => {
            if (selectedProjects.length === filteredProjects.length) {
              setSelected(new Set());
            } else {
              setSelected(new Set(filteredProjects.map((p) => p.id)));
            }
          }}
          className="text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          {selectedProjects.length === filteredProjects.length ? "取消全选" : "全选"}
        </button>
        <span className="text-sm text-foreground">已选 {selectedProjects.length}</span>
        <button
          onClick={() => setBatchDeleteOpen(true)}
          disabled={batchBusy || selectedProjects.length === 0}
          className="text-xs px-3 py-1.5 rounded-md bg-destructive text-destructive-foreground hover:bg-destructive/90 transition-colors flex items-center gap-1.5 disabled:opacity-50"
        >
          {batchBusy ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Trash2 className="w-3.5 h-3.5" />}
          删除选中
        </button>
        <button
          onClick={exitSelectMode}
          className="text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          退出
        </button>
      </div>
    )}

    {/* 批量删除项目确认 */}
    {batchDeleteOpen && (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
        <div className="bg-card border border-border rounded-lg p-6 max-w-sm w-full mx-4 shadow-lg">
          <h3 className="text-lg font-semibold mb-2">批量删除工程</h3>
          <p className="text-sm text-muted-foreground mb-4">
            将删除选中的 {selectedProjects.length} 个工程的会话记录（移入回收站，可在回收站还原）。
          </p>

          {source === "claude" && (
            <label className="flex items-start gap-2 cursor-pointer mb-4 group">
              <input
                type="checkbox"
                checked={batchWithCcConfig}
                onChange={(e) => setBatchWithCcConfig(e.target.checked)}
                disabled={batchBusy}
                className="mt-0.5 accent-destructive"
              />
              <span className="text-xs text-muted-foreground leading-relaxed">
                同时清理 Claude Code 项目配置
                {batchWithCcConfig && (
                  <span className="block mt-1 text-yellow-600 dark:text-yellow-400">
                    将从 ~/.claude.json 移除这些项目配置
                  </span>
                )}
              </span>
            </label>
          )}

          <div className="flex justify-end gap-2">
            <button
              onClick={() => { setBatchDeleteOpen(false); setBatchWithCcConfig(false); }}
              disabled={batchBusy}
              className="px-4 py-2 text-sm rounded-md border border-border hover:bg-accent transition-colors"
            >
              取消
            </button>
            <button
              onClick={handleBatchDelete}
              disabled={batchBusy}
              className="px-4 py-2 text-sm rounded-md bg-destructive text-destructive-foreground hover:bg-destructive/90 transition-colors flex items-center gap-1.5 disabled:opacity-50"
            >
              {batchBusy && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
              {batchBusy ? "删除中..." : "删除"}
            </button>
          </div>
        </div>
      </div>
    )}
    </>
  );
}
