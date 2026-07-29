import { useEffect, useState, useMemo } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { useAppStore } from "../../stores/appStore";
import { useChatStore } from "../../stores/chatStore";
import { api } from "../../services/api";
import type { ModelInfo } from "../../types/chat";
import { useTheme } from "../../hooks/useTheme";
import { useUpdateChecker } from "../../hooks/useUpdateChecker";
import { useBackgroundRefresh } from "../../hooks/useBackgroundRefresh";
import { useFileWatcher } from "../../hooks/useFileWatcher";
import { UpdateIndicator } from "./UpdateIndicator";
import { ProjectActionsMenu } from "../project/ProjectActionsMenu";
import { DeleteProjectDialog } from "../project/DeleteProjectDialog";
import type { ProjectEntry } from "../../types";
import { collapseDirectBuckets, DIRECT_GROUP_ID } from "../../utils/directChat";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import {
  FolderOpen,
  FolderClock,
  Search,
  BarChart3,
  MoreHorizontal,
  Bot,
  Terminal,
  Sun,
  Moon,
  Monitor,
  Settings,
  X,
  Mail,
  Users,
  Github,
  ExternalLink,
  MessageSquarePlus,
  RefreshCw,
  Plus,
  Trash2,
  Check,
  Copy,
  Loader2,
  AlertCircle,
  Star,
  FolderX,
  Repeat,
  Sparkles,
} from "lucide-react";

declare const __IS_TAURI__: boolean;
declare const __APP_VERSION__: string;

export function Sidebar() {
  const navigate = useNavigate();
  const location = useLocation();
  const { source, setSource, projects, loadProjects, projectsLoading, bookmarks, loadBookmarks, deleteProject, setProjectAlias, recycledItems } =
    useAppStore();
  const { theme, setTheme } = useTheme();
  const { detectCli, availableClis, clearChat } = useChatStore();
  const [showSettings, setShowSettings] = useState(false);
  const [settingsTab, setSettingsTab] = useState<"guide" | "chat" | "update" | "about">("guide");
  const [projectActionsMenu, setProjectActionsMenu] = useState<{
    project: ProjectEntry;
    anchorRect: DOMRect;
  } | null>(null);
  const [renameTarget, setRenameTarget] = useState<ProjectEntry | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [renameError, setRenameError] = useState<string | null>(null);
  const [renameLoading, setRenameLoading] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<ProjectEntry | null>(null);
  useUpdateChecker();
  useBackgroundRefresh();
  useFileWatcher();

  useEffect(() => {
    loadProjects();
    loadBookmarks();
  }, [source]);

  useEffect(() => {
    // CLI detection only matters once the user opens a chat/session; defer it
    // off the startup burst so it doesn't compete with the first project load.
    if (availableClis.length > 0) return;
    const t = setTimeout(() => {
      if (useChatStore.getState().availableClis.length === 0) {
        detectCli();
      }
    }, 0);
    return () => clearTimeout(t);
  }, []);

  const openExternal = async (url: string) => {
    if (__IS_TAURI__) {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(url);
    } else {
      window.open(url, "_blank");
    }
  };

  const isActive = (path: string) => location.pathname === path;
  const isProjectActive = (projectId: string) =>
    location.pathname.startsWith(`/projects/${encodeURIComponent(projectId)}`);

  // Collapse Codex direct-chat date buckets into one pinned "直连对话" entry,
  // mirroring the main projects grid.
  const sidebarProjects = useMemo(() => collapseDirectBuckets(projects), [projects]);
  // The aggregate entry and the date-list page are "active" together; so is any
  // drill-down into a `<codex-direct>/DATE` bucket's session list.
  const isDirectGroupActive =
    location.pathname === "/direct-chat" ||
    location.pathname.startsWith(`/projects/${encodeURIComponent("<codex-direct>/")}`);

  const handleSourceChange = (s: "claude" | "codex" | "grok") => {
    if (s !== source) {
      setSource(s);
      navigate("/projects");
    }
  };

  return (
    <aside className="w-64 h-full border-r border-border bg-card flex flex-col shrink-0">
      {/* Header */}
      <div className="p-4 border-b border-border">
        <h1 className="text-sm font-semibold text-foreground mb-3 text-center">
          AI Session Viewer
        </h1>
        {/* Source Tabs */}
        <div className="flex rounded-lg bg-muted p-0.5">
          <button
            onClick={() => handleSourceChange("claude")}
            className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all ${
              source === "claude"
                ? "bg-orange-500/20 text-orange-400 shadow-sm"
                : "text-muted-foreground hover:text-foreground"
            }`}
          >
            <Bot className="w-3.5 h-3.5" />
            Claude
          </button>
          <button
            onClick={() => handleSourceChange("codex")}
            className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all ${
              source === "codex"
                ? "bg-green-500/20 text-green-400 shadow-sm"
                : "text-muted-foreground hover:text-foreground"
            }`}
          >
            <Terminal className="w-3.5 h-3.5" />
            Codex
          </button>
          <button
            onClick={() => handleSourceChange("grok")}
            className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all ${
              source === "grok"
                ? "bg-purple-500/20 text-purple-400 shadow-sm"
                : "text-muted-foreground hover:text-foreground"
            }`}
          >
            <Sparkles className="w-3.5 h-3.5" />
            Grok
          </button>
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 overflow-y-auto p-2">
        {/* Quick links */}
        <div className="mb-4">
          {source !== "grok" && (
            <button
              onClick={() => {
                clearChat();
                navigate("/chat");
              }}
              className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
                location.pathname.startsWith("/chat")
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
              }`}
            >
              <MessageSquarePlus className="w-4 h-4" />
              CLI 对话
            </button>
          )}
          <button
            onClick={() => navigate("/search")}
            className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
              isActive("/search")
                ? "bg-accent text-accent-foreground"
                : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
            }`}
          >
            <Search className="w-4 h-4" />
            全局搜索
          </button>
          {source !== "grok" && (
            <button
              onClick={() => navigate("/stats")}
              className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
                isActive("/stats")
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
              }`}
            >
              <BarChart3 className="w-4 h-4" />
              使用统计
            </button>
          )}
          <button
            onClick={() => navigate("/skills")}
            className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
              isActive("/skills")
                ? "bg-accent text-accent-foreground"
                : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
            }`}
          >
            <Sparkles className="w-4 h-4" />
            Skills
          </button>
          <button
            onClick={() => navigate("/bookmarks")}
            className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
              isActive("/bookmarks")
                ? "bg-accent text-accent-foreground"
                : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
            }`}
          >
            <Star className="w-4 h-4" />
            收藏
            {bookmarks.filter((b) => b.source === source).length > 0 && (
              <span className="ml-auto text-xs bg-yellow-500/20 text-yellow-500 px-1.5 py-0.5 rounded-full">
                {bookmarks.filter((b) => b.source === source).length}
              </span>
            )}
          </button>
          <button
            onClick={() => navigate("/cleanup")}
            className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
              isActive("/cleanup")
                ? "bg-accent text-accent-foreground"
                : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
            }`}
          >
            <FolderX className="w-4 h-4" />
            无效项管理
          </button>
          <button
            onClick={() => navigate("/recyclebin")}
            className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
              isActive("/recyclebin")
                ? "bg-accent text-accent-foreground"
                : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
            }`}
          >
            <Trash2 className="w-4 h-4" />
            回收站
            {recycledItems.length > 0 && (
              <span className="ml-auto text-xs bg-muted text-muted-foreground px-1.5 py-0.5 rounded-full">
                {recycledItems.length}
              </span>
            )}
          </button>
          {source === "codex" && (
            <button
              onClick={() => navigate("/provider-sync")}
              className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
                isActive("/provider-sync")
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
              }`}
            >
              <Repeat className="w-4 h-4" />
              Provider 同步
            </button>
          )}
        </div>

        {/* Projects list */}
        <div>
          <h2 className="px-3 py-1 text-xs font-medium text-muted-foreground uppercase tracking-wider">
            项目 ({sidebarProjects.length})
          </h2>
          {projectsLoading ? (
            <div className="px-3 py-2 text-sm text-muted-foreground">
              加载中...
            </div>
          ) : (
            <div className="mt-1 space-y-0.5">
              {sidebarProjects.map((project) => {
                const isGroup = project.id === DIRECT_GROUP_ID;
                const active = isGroup
                  ? isDirectGroupActive
                  : isProjectActive(project.id);
                return (
                <div
                  key={project.id}
                  className={`relative flex items-center rounded-md text-sm transition-colors group ${
                    active
                      ? "bg-accent text-accent-foreground"
                      : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                  }`}
                >
                  <button
                    onClick={() =>
                      navigate(
                        isGroup
                          ? "/direct-chat"
                          : `/projects/${encodeURIComponent(project.id)}`
                      )
                    }
                    className="flex-1 flex items-center gap-2 px-3 py-1.5 min-w-0"
                    title={
                      project.isVirtual
                        ? `${project.displayPath}（按日期归类的虚拟项目）`
                        : project.displayPath + (project.pathExists === false ? " (路径不存在)" : "")
                    }
                  >
                    {project.isVirtual ? (
                      <FolderClock className="w-3.5 h-3.5 shrink-0 text-muted-foreground/70" />
                    ) : (
                      <FolderOpen className={`w-3.5 h-3.5 shrink-0${project.pathExists === false ? " text-yellow-500" : ""}`} />
                    )}
                    <span className="truncate flex-1 text-left">
                      {project.alias ?? project.shortName}
                    </span>
                    <span className="text-xs text-muted-foreground shrink-0">
                      {project.sessionCount}
                    </span>
                  </button>
                  {!isGroup && (
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setProjectActionsMenu({
                          project,
                          anchorRect: e.currentTarget.getBoundingClientRect(),
                        });
                      }}
                      className="opacity-0 group-hover:opacity-100 transition-opacity p-1.5 mr-1 rounded text-muted-foreground hover:bg-accent/50 shrink-0"
                      title="操作"
                    >
                      <MoreHorizontal className="w-3.5 h-3.5" />
                    </button>
                  )}
                </div>
                );
              })}
            </div>
          )}
        </div>
      </nav>

      {/* Footer */}
      <div className="p-3 border-t border-border">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">
              v{__APP_VERSION__}
            </span>
            <button
              onClick={() => setShowSettings(true)}
              className="p-1 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
              title="设置"
            >
              <Settings className="w-3.5 h-3.5" />
            </button>
          </div>
          <div className="flex rounded-md bg-muted p-0.5">
            <button
              onClick={() => setTheme("light")}
              className={`p-1 rounded transition-colors ${
                theme === "light" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
              }`}
              title="亮色模式"
            >
              <Sun className="w-3.5 h-3.5" />
            </button>
            <button
              onClick={() => setTheme("system")}
              className={`p-1 rounded transition-colors ${
                theme === "system" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
              }`}
              title="跟随系统"
            >
              <Monitor className="w-3.5 h-3.5" />
            </button>
            <button
              onClick={() => setTheme("dark")}
              className={`p-1 rounded transition-colors ${
                theme === "dark" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
              }`}
              title="暗色模式"
            >
              <Moon className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      </div>

      {/* Settings Modal */}
      {showSettings && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
          onClick={() => setShowSettings(false)}
        >
          <div
            className="bg-card border border-border rounded-lg shadow-lg w-[28rem] max-w-[90vw]"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between p-4 border-b border-border">
              <h2 className="text-sm font-semibold text-foreground">设置</h2>
              <button
                onClick={() => setShowSettings(false)}
                className="p-1 rounded transition-colors text-muted-foreground hover:text-foreground hover:bg-accent/50"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            {/* Tabs */}
            <div className="flex border-b border-border">
              <button
                onClick={() => setSettingsTab("guide")}
                className={`flex-1 px-4 py-2 text-sm font-medium transition-colors ${
                  settingsTab === "guide"
                    ? "text-foreground border-b-2 border-foreground"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                使用说明
              </button>
              <button
                onClick={() => setSettingsTab("chat")}
                className={`flex-1 px-4 py-2 text-sm font-medium transition-colors ${
                  settingsTab === "chat"
                    ? "text-foreground border-b-2 border-foreground"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                对话设置
              </button>
              {__IS_TAURI__ && (
                <button
                  onClick={() => setSettingsTab("update")}
                  className={`flex-1 px-4 py-2 text-sm font-medium transition-colors ${
                    settingsTab === "update"
                      ? "text-foreground border-b-2 border-foreground"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                >
                  更新检查
                </button>
              )}
              <button
                onClick={() => setSettingsTab("about")}
                className={`flex-1 px-4 py-2 text-sm font-medium transition-colors ${
                  settingsTab === "about"
                    ? "text-foreground border-b-2 border-foreground"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                关于作者
              </button>
            </div>
            {/* Body */}
            <div className="max-h-[70vh] overflow-y-auto">
              {settingsTab === "chat" ? (
                <ChatSettingsTab />
              ) : settingsTab === "update" && __IS_TAURI__ ? (
                <div className="p-4">
                  <UpdateIndicator />
                </div>
              ) : settingsTab === "guide" ? (
                <div className="p-4 space-y-4 text-sm text-foreground">
                  <section>
                    <h3 className="font-medium mb-1.5">侧边栏</h3>
                    <ul className="list-disc list-inside space-y-1 text-muted-foreground">
                      <li>顶部 Tab 切换数据源（Claude / Codex）</li>
                      <li>项目列表点击进入对应项目的会话列表</li>
                      <li>快捷入口：全局搜索、使用统计、无效项管理、回收站</li>
                    </ul>
                  </section>
                  <section>
                    <h3 className="font-medium mb-1.5">项目列表</h3>
                    <ul className="list-disc list-inside space-y-1 text-muted-foreground">
                      <li>标签 pill 可筛选项目</li>
                      <li>点击项目卡片进入会话列表</li>
                    </ul>
                  </section>
                  <section>
                    <h3 className="font-medium mb-1.5">会话列表</h3>
                    <ul className="list-disc list-inside space-y-1 text-muted-foreground">
                      <li>点击卡片查看消息详情</li>
                      <li>悬停显示操作：🏷编辑标签、▶Resume、复制命令、🗑删除</li>
                      <li>桌面端可直接点 Resume，或点“复制命令”到剪贴板</li>
                      <li>标签筛选快速定位会话</li>
                    </ul>
                  </section>
                  <section>
                    <h3 className="font-medium mb-1.5">消息详情</h3>
                    <ul className="list-disc list-inside space-y-1 text-muted-foreground">
                      <li>向上滚动自动加载更早消息</li>
                      <li>顶栏可切换时间戳 / 模型显示</li>
                      <li>浮动按钮快速跳转到顶部或底部</li>
                    </ul>
                  </section>
                  <section>
                    <h3 className="font-medium mb-1.5">全局搜索</h3>
                    <ul className="list-disc list-inside space-y-1 text-muted-foreground">
                      <li>输入关键词跨项目搜索消息</li>
                      <li>标签筛选缩小搜索范围</li>
                      <li>点击结果直接跳转到对应消息</li>
                    </ul>
                  </section>
                  <section>
                    <h3 className="font-medium mb-1.5">主题切换</h3>
                    <ul className="list-disc list-inside space-y-1 text-muted-foreground">
                      <li>底部按钮组切换亮色 / 暗色 / 跟随系统</li>
                    </ul>
                  </section>
                </div>
              ) : (
                <div className="p-4 space-y-3">
                  <div className="flex items-center gap-2.5 text-sm text-foreground">
                    <Users className="w-4 h-4 text-muted-foreground shrink-0" />
                    <span>作者：左岚</span>
                  </div>
                  <button
                    onClick={() => openExternal("mailto:zuolan1102@qq.com")}
                    className="flex items-center gap-2.5 text-sm text-foreground hover:text-accent-foreground transition-colors"
                  >
                    <Mail className="w-4 h-4 text-muted-foreground shrink-0" />
                    <span>zuolan1102@qq.com</span>
                  </button>
                  <div className="flex items-center gap-2.5 text-sm text-foreground">
                    <svg className="w-4 h-4 text-muted-foreground shrink-0" viewBox="0 0 24 24" fill="currentColor">
                      <path d="M21.395 15.035a39.548 39.548 0 0 0-1.51-3.302c-.18-.348-.478-.81-.478-.81s.09-.604.192-1.044c.118-.502.143-.878.143-1.37 0-2.737-1.94-5.057-4.96-5.057-1.063 0-2.044.291-2.893.812-.38-.133-.78-.232-1.198-.298a10.71 10.71 0 0 0-.93-.09c-.213-.01-.432-.013-.623-.002-.39.021-.72.068-.72.068s-.29-.012-.603.063c-.26.064-.505.15-.74.266A5.422 5.422 0 0 0 4.25 9.498c0 .608.106 1.178.3 1.698a8.38 8.38 0 0 0-.353.638c-.394.811-.64 1.727-.64 2.678 0 3.456 2.727 5.94 6.262 5.94.857 0 1.67-.14 2.42-.395.324.085.67.14 1.03.162.196.01.404.006.61-.008.37-.027.68-.071.68-.071s.25.021.54-.048c.244-.058.471-.137.692-.241a5.082 5.082 0 0 0 2.804-4.623c0-.493-.074-.961-.21-1.397.275-.376.524-.776.746-1.196zm-5.905 4.238c-.522.063-1.084-.129-1.084-.129s-.254.09-.558.127a3.282 3.282 0 0 1-.467.018 2.58 2.58 0 0 1-.519-.062c-.186-.049-.37-.12-.37-.12s-.478.136-.886.096c-1.863-.181-3.26-1.467-3.26-3.292 0-.375.07-.728.194-1.052.247-.634.72-1.168 1.343-1.518.703-.395 1.622-.584 2.732-.482.32.03.628.084.918.162.442-.285.957-.464 1.51-.502.062-.004.126-.005.189-.003.063.003.127.01.193.02 1.612.234 2.754 1.578 2.754 3.173 0 1.78-1.31 3.37-2.689 3.564z" />
                    </svg>
                    <span>QQ 群：1019721429</span>
                  </div>
                  <button
                    onClick={() => openExternal("https://space.bilibili.com/27619688")}
                    className="flex items-center gap-2.5 text-sm text-foreground hover:text-accent-foreground transition-colors"
                  >
                    <svg className="w-4 h-4 text-muted-foreground shrink-0" viewBox="0 0 24 24" fill="currentColor">
                      <path d="M17.813 4.653h.854c1.51.054 2.769.578 3.773 1.574 1.004.995 1.524 2.249 1.56 3.76v7.36c-.036 1.51-.556 2.769-1.56 3.773s-2.262 1.524-3.773 1.56H5.333c-1.51-.036-2.769-.556-3.773-1.56S.036 18.858 0 17.347v-7.36c.036-1.511.556-2.765 1.56-3.76 1.004-.996 2.262-1.52 3.773-1.574h.774l-1.174-1.12a1.234 1.234 0 0 1-.373-.906c0-.356.124-.658.373-.907l.027-.027c.267-.249.573-.373.92-.373.347 0 .653.124.92.373L9.653 4.44c.071.071.134.142.187.213h4.267a.836.836 0 0 1 .16-.213l2.853-2.747c.267-.249.573-.373.92-.373.347 0 .662.151.929.4.267.249.391.551.391.907 0 .355-.124.657-.373.906zM5.333 7.24c-.746.018-1.373.276-1.88.773-.506.498-.769 1.13-.787 1.893v7.44c.018.764.281 1.395.787 1.893.507.498 1.134.756 1.88.773h13.334c.746-.017 1.373-.275 1.88-.773.506-.498.769-1.129.787-1.893v-7.44c-.018-.764-.281-1.395-.787-1.893a2.51 2.51 0 0 0-1.88-.773zM8 11.107c.373 0 .684.124.933.373.25.249.383.569.4.96v1.173c-.017.391-.15.711-.4.96-.249.25-.56.374-.933.374s-.684-.125-.933-.374c-.25-.249-.383-.569-.4-.96V12.44c.017-.391.15-.711.4-.96.249-.249.56-.373.933-.373zm8 0c.373 0 .684.124.933.373.25.249.383.569.4.96v1.173c-.017.391-.15.711-.4.96-.249.25-.56.374-.933.374s-.684-.125-.933-.374c-.25-.249-.383-.569-.4-.96V12.44c.017-.391.15-.711.4-.96.249-.249.56-.373.933-.373z" />
                    </svg>
                    <span>哔哩哔哩</span>
                    <ExternalLink className="w-3 h-3 text-muted-foreground" />
                  </button>
                  <button
                    onClick={() => openExternal("https://github.com/zuoliangyu/AI-Session-Viewer")}
                    className="flex items-center gap-2.5 text-sm text-foreground hover:text-accent-foreground transition-colors"
                  >
                    <Github className="w-4 h-4 text-muted-foreground shrink-0" />
                    <span>GitHub</span>
                    <ExternalLink className="w-3 h-3 text-muted-foreground" />
                  </button>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
      {/* ⋯ 菜单 portal */}
      {projectActionsMenu && (
        <ProjectActionsMenu
          project={projectActionsMenu.project}
          source={source}
          anchorRect={projectActionsMenu.anchorRect}
          onClose={() => setProjectActionsMenu(null)}
          onRename={(p) => {
            setRenameTarget(p);
            setRenameValue(p.alias ?? "");
            setRenameError(null);
          }}
          onDelete={(p) => { setDeleteTarget(p); }}
        />
      )}

      {/* 重命名 Modal */}
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
                if (e.key === "Escape") { setRenameTarget(null); setRenameError(null); }
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

      {/* 删除确认 Modal */}
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
    </aside>
  );
}

function ProviderModelManager({ source }: { source: "claude" | "codex" }) {
  const {
    addCustomModel,
    removeCustomModel,
    claudeApiKeyOverride,
    claudeBaseUrlOverride,
    codexApiKeyOverride,
    codexBaseUrlOverride,
  } = useChatStore();

  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fetched, setFetched] = useState(false);
  const [showAddInput, setShowAddInput] = useState(false);
  const [newModelIds, setNewModelIds] = useState("");
  const [addedCount, setAddedCount] = useState<number | null>(null);

  const apiKey = source === "codex" ? codexApiKeyOverride : claudeApiKeyOverride;
  const baseUrl = source === "codex" ? codexBaseUrlOverride : claudeBaseUrlOverride;

  // Custom model IDs for this source (re-read whenever models change)
  const customModelIds = useMemo(() => {
    try {
      return new Set<string>(JSON.parse(localStorage.getItem(`chat_customModels_${source}`) || "[]"));
    } catch {
      return new Set<string>();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [models, source]);

  const loadModels = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await api.listModels(source, apiKey, baseUrl);
      // Prepend persisted custom models not already in result
      const customKey = `chat_customModels_${source}`;
      const customIds: string[] = JSON.parse(localStorage.getItem(customKey) || "[]");
      const resultIds = new Set(result.map((m) => m.id));
      const provider = source === "codex" ? "openai" : "anthropic";
      const extras: ModelInfo[] = customIds
        .filter((id) => !resultIds.has(id))
        .map((id) => ({ id, name: id, provider, group: "自定义", created: null }));
      setModels([...extras, ...result]);
      setFetched(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setFetched(true);
    } finally {
      setLoading(false);
    }
  };

  const parseModelIds = (input: string): string[] =>
    input.split(/[,;\n]+/).map((s) => s.trim()).filter(Boolean);

  const handleBatchAdd = () => {
    const ids = parseModelIds(newModelIds);
    if (ids.length === 0) return;
    let count = 0;
    const provider = source === "codex" ? "openai" : "anthropic";
    for (const id of ids) {
      if (!models.some((m) => m.id === id)) {
        addCustomModel(id, source);
        setModels((prev) => [{ id, name: id, provider, group: "自定义", created: null }, ...prev]);
        count++;
      }
    }
    setAddedCount(count);
    setNewModelIds("");
    setShowAddInput(false);
    setTimeout(() => setAddedCount(null), 2000);
  };

  const handleRemove = (id: string) => {
    removeCustomModel(id, source);
    setModels((prev) => prev.filter((m) => m.id !== id));
  };

  const parsedCount = parseModelIds(newModelIds).length;

  const grouped = useMemo(() => {
    const groups: Record<string, ModelInfo[]> = {};
    for (const m of models) {
      if (!groups[m.group]) groups[m.group] = [];
      groups[m.group].push(m);
    }
    return groups;
  }, [models]);

  return (
    <div className="mt-3 space-y-2">
      {/* Action row */}
      <div className="flex items-center gap-1.5 flex-wrap">
        <button
          onClick={loadModels}
          disabled={loading}
          className="flex items-center gap-1 px-2 py-1 text-xs rounded border border-border bg-muted text-foreground hover:bg-accent/50 transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-3 h-3 ${loading ? "animate-spin" : ""}`} />
          {fetched ? "刷新模型列表" : "获取模型列表"}
        </button>
        <button
          onClick={() => { setShowAddInput((v) => !v); setAddedCount(null); }}
          className={`flex items-center gap-1 px-2 py-1 text-xs rounded border transition-colors ${
            showAddInput
              ? "border-primary bg-primary/10 text-primary"
              : "border-border bg-muted text-foreground hover:bg-accent/50"
          }`}
        >
          <Plus className="w-3 h-3" />
          手动添加
        </button>
        {fetched && !loading && (
          <span className="text-[10px] text-muted-foreground ml-auto">
            {models.length} 个模型
            {customModelIds.size > 0 && `（${customModelIds.size} 个自定义）`}
          </span>
        )}
      </div>

      {/* Added toast */}
      {addedCount !== null && (
        <div className="flex items-center gap-1 text-xs text-green-500">
          <Check className="w-3 h-3" />
          已添加 {addedCount} 个自定义模型
        </div>
      )}

      {/* Batch add textarea */}
      {showAddInput && (
        <div className="space-y-1.5">
          <textarea
            value={newModelIds}
            onChange={(e) => setNewModelIds(e.target.value)}
            onKeyDown={(e) => {
              if ((e.ctrlKey || e.metaKey) && e.key === "Enter") { e.preventDefault(); handleBatchAdd(); }
              if (e.key === "Escape") { e.preventDefault(); setShowAddInput(false); }
            }}
            placeholder={
              source === "codex"
                ? "每行一个模型 ID，例如：\no4-mini\ngpt-4.1\ncustom-model-id"
                : "每行一个模型 ID，例如：\nclaude-sonnet-4-20250514\nclaude-opus-4-20250514"
            }
            rows={4}
            className="w-full bg-muted border border-border rounded px-2 py-1.5 text-xs font-mono text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary resize-none"
            autoFocus
          />
          <div className="flex items-center justify-between">
            <span className="text-[10px] text-muted-foreground">
              {parsedCount > 0 ? `识别到 ${parsedCount} 个 ID` : "每行一个或逗号分隔"}
            </span>
            <button
              onClick={handleBatchAdd}
              disabled={parsedCount === 0}
              className="px-2 py-1 text-xs rounded bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
            >
              添加{parsedCount > 0 ? `（${parsedCount}）` : ""}
            </button>
          </div>
        </div>
      )}

      {/* Status */}
      {loading && (
        <div className="flex items-center gap-1.5 py-1 text-xs text-muted-foreground">
          <Loader2 className="w-3 h-3 animate-spin" />
          正在获取...
        </div>
      )}
      {error && (
        <div className="flex items-center gap-1.5 py-1 text-xs text-red-400">
          <AlertCircle className="w-3 h-3 shrink-0" />
          <span className="truncate">{error}</span>
        </div>
      )}
      {fetched && !loading && models.length === 0 && !error && (
        <p className="text-xs text-muted-foreground py-1">
          未获取到模型。{!apiKey && "请先配置 API Key 或手动输入覆盖值。"}
        </p>
      )}

      {/* Model list */}
      {fetched && !loading && models.length > 0 && (
        <div className="border border-border rounded overflow-hidden max-h-52 overflow-y-auto">
          {Object.entries(grouped).map(([group, grpModels]) => (
            <div key={group}>
              <div className="px-2 py-0.5 text-[10px] font-semibold text-muted-foreground uppercase tracking-wider bg-muted/60 sticky top-0">
                {group}
                <span className="ml-1 font-normal opacity-60">({grpModels.length})</span>
              </div>
              {grpModels.map((m) => {
                const isCustom = customModelIds.has(m.id);
                return (
                  <div
                    key={m.id}
                    className="flex items-center gap-1.5 px-2 py-1 text-xs hover:bg-accent/30 transition-colors group"
                  >
                    <span className="truncate flex-1 text-foreground" title={m.id}>{m.name}</span>
                    {m.id !== m.name && (
                      <span className="text-[10px] text-muted-foreground/70 truncate max-w-[9rem] font-mono">{m.id}</span>
                    )}
                    {isCustom ? (
                      <button
                        onClick={() => handleRemove(m.id)}
                        className="p-0.5 rounded text-transparent group-hover:text-muted-foreground hover:!text-red-400 transition-colors shrink-0"
                        title="移除自定义模型"
                      >
                        <Trash2 className="w-3 h-3" />
                      </button>
                    ) : (
                      <span className="w-4 shrink-0" />
                    )}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function CliConfigInfo({
  config,
  loading,
  error,
  onFetch,
}: {
  config: import("../../types/chat").CliConfig | null;
  loading: boolean;
  error: string | null;
  onFetch: () => void;
}) {
  if (loading) {
    return (
      <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
        <Loader2 className="w-3 h-3 animate-spin" />
        检测中...
      </div>
    );
  }
  if (error) {
    return (
      <div className="space-y-1">
        <div className="flex items-center gap-1.5 text-xs text-red-400">
          <AlertCircle className="w-3 h-3" />
          {error}
        </div>
        <button onClick={onFetch} className="text-xs text-primary hover:text-primary/80">重试</button>
      </div>
    );
  }
  if (!config) {
    return <button onClick={onFetch} className="text-xs text-primary hover:text-primary/80">检测配置</button>;
  }

  const isCodex = config.source === "codex";

  return (
    <div className="space-y-2 text-xs">
      {isCodex ? (
        // ── Codex: show both files separately ──
        <>
          {/* auth.json */}
          <div className="rounded-md border border-border bg-muted/40 px-2.5 py-2 space-y-1">
            <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground font-medium uppercase tracking-wide">
              <span>auth.json</span>
              <span className="font-normal opacity-60 truncate">{config.authJsonPath}</span>
            </div>
            <div className="flex items-center gap-2">
              <div className={`w-1.5 h-1.5 rounded-full shrink-0 ${config.authJsonHasKey ? "bg-green-500" : "bg-yellow-500"}`} />
              <span className="text-muted-foreground">API Key:</span>
              <span className="font-mono text-foreground">
                {config.authJsonHasKey ? config.authJsonKeyMasked : "未找到"}
              </span>
              {config.authJsonHasKey && config.apiKeySource === "auth.json" && (
                <span className="ml-auto text-[10px] text-green-600 dark:text-green-400">✓ 使用中</span>
              )}
            </div>
          </div>

          {/* config.toml */}
          <div className="rounded-md border border-border bg-muted/40 px-2.5 py-2 space-y-1">
            <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground font-medium uppercase tracking-wide">
              <span>config.toml</span>
              <span className="font-normal opacity-60 truncate">{config.configPath}</span>
            </div>
            <div className="flex items-center gap-2">
              <div className={`w-1.5 h-1.5 rounded-full shrink-0 ${config.configTomlUrl ? "bg-green-500" : "bg-yellow-500"}`} />
              <span className="text-muted-foreground">Base URL:</span>
              <span className="font-mono text-foreground truncate">
                {config.configTomlUrl || "未找到（将用默认值）"}
              </span>
              {config.configTomlUrl && config.baseUrlSource === "config.toml" && (
                <span className="ml-auto shrink-0 text-[10px] text-green-600 dark:text-green-400">✓ 使用中</span>
              )}
            </div>
            {config.configTomlHasKey && (
              <div className="flex items-center gap-2">
                <div className="w-1.5 h-1.5 rounded-full shrink-0 bg-blue-400" />
                <span className="text-muted-foreground">API Key:</span>
                <span className="font-mono text-foreground">{config.configTomlKeyMasked}</span>
                {config.apiKeySource === "config.toml" && (
                  <span className="ml-auto text-[10px] text-green-600 dark:text-green-400">✓ 使用中</span>
                )}
              </div>
            )}
            {config.defaultModel && (
              <div className="flex items-center gap-2">
                <span className="w-1.5 shrink-0" />
                <span className="text-muted-foreground">默认模型:</span>
                <span className="font-mono text-foreground">{config.defaultModel}</span>
              </div>
            )}
          </div>

          {/* Resolved summary */}
          {!config.hasApiKey && (
            <p className="text-[11px] text-yellow-500">
              未找到 API Key，请在 auth.json 中配置或在下方手动填入。
            </p>
          )}
          {config.baseUrlSource === "default" && (
            <p className="text-[11px] text-muted-foreground">
              Base URL 使用默认值：{config.baseUrl}
            </p>
          )}
        </>
      ) : (
        // ── Claude: simple display ──
        <>
          <div className="flex items-center gap-2">
            <div className={`w-1.5 h-1.5 rounded-full ${config.hasApiKey ? "bg-green-500" : "bg-red-500"}`} />
            <span className="text-muted-foreground">API Key:</span>
            <span className="font-mono text-foreground">{config.hasApiKey ? config.apiKeyMasked : "未配置"}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="ml-3.5 text-muted-foreground">Base URL:</span>
            <span className="font-mono text-foreground truncate">{config.baseUrl}</span>
          </div>
          {config.defaultModel && (
            <div className="flex items-center gap-2">
              <span className="ml-3.5 text-muted-foreground">默认模型:</span>
              <span className="font-mono text-foreground">{config.defaultModel}</span>
            </div>
          )}
          <div className="flex items-center gap-2">
            <span className="ml-3.5 text-muted-foreground">配置文件:</span>
            <span className="text-foreground/60 font-mono truncate text-[10px]">{config.configPath}</span>
          </div>
        </>
      )}

      <button onClick={onFetch} className="text-xs text-primary hover:text-primary/80">
        重新检测
      </button>
    </div>
  );
}

function CliConfigDisplay() {
  const { cliConfig, cliConfigLoading, cliConfigError, fetchCliConfig } = useChatStore();
  const [fetched, setFetched] = useState(false);
  const handleFetch = async () => { await fetchCliConfig(); setFetched(true); };
  useEffect(() => { if (!fetched) handleFetch(); }, []); // eslint-disable-line react-hooks/exhaustive-deps
  return <CliConfigInfo config={cliConfig} loading={cliConfigLoading} error={cliConfigError} onFetch={handleFetch} />;
}

function CodexCliConfigDisplay() {
  const { codexCliConfig, codexCliConfigLoading, codexCliConfigError, fetchCodexCliConfig } = useChatStore();
  const [fetched, setFetched] = useState(false);
  const handleFetch = async () => { await fetchCodexCliConfig(); setFetched(true); };
  useEffect(() => { if (!fetched) handleFetch(); }, []); // eslint-disable-line react-hooks/exhaustive-deps
  return <CliConfigInfo config={codexCliConfig} loading={codexCliConfigLoading} error={codexCliConfigError} onFetch={handleFetch} />;
}

type InstallMethod = "npm" | "nvm" | "bun" | "other";

const INSTALL_HINTS: Record<InstallMethod, { label: string; paths: string[]; tip: string }> = {
  npm: {
    label: "npm 全局安装",
    paths: [
      "Windows: %APPDATA%\\npm\\claude.cmd",
      "Mac/Linux: ~/.npm-global/bin/claude 或 /usr/local/bin/claude",
    ],
    tip: "在终端运行 `npm list -g @anthropic-ai/claude-code` 确认安装，再用 `which claude`（Mac/Linux）或 `where claude`（Windows）获取实际路径，填入下方「CLI 路径」",
  },
  nvm: {
    label: "nvm (all platforms)",
    paths: [
      "Mac/Linux: ~/.nvm/versions/node/{version}/bin/claude",
      "Windows (nvm-windows): %APPDATA%\\nvm\\{version}\\claude.cmd",
    ],
    tip: "由于桌面应用不继承 shell 的 nvm PATH，自动检测可能失败。请在终端执行 `nvm use` 激活版本后运行 `which claude`（Mac/Linux）或 `where claude`（Windows），将完整路径填入下方「CLI 路径」",
  },
  bun: {
    label: "bun 全局安装",
    paths: [
      "Mac/Linux: ~/.bun/bin/claude",
      "Windows: %USERPROFILE%\\.bun\\bin\\claude.exe",
    ],
    tip: "在终端运行 `bun pm ls -g` 确认安装，再将 `~/.bun/bin/claude` 填入下方「CLI 路径」",
  },
  other: {
    label: "手动 / 其他",
    paths: ["自定义路径"],
    tip: "在终端运行 `which claude`（Mac/Linux）或 `where claude`（Windows）获取路径，填入下方「CLI 路径」",
  },
};

function CopyCommandLine({ cmd }: { cmd: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="flex items-center gap-1.5 bg-muted/60 rounded px-2 py-1">
      <code className="flex-1 text-[11px] font-mono text-foreground select-all">{cmd}</code>
      <button
        onClick={() => {
          navigator.clipboard.writeText(cmd);
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        }}
        className="shrink-0 text-muted-foreground hover:text-foreground transition-colors"
        title="复制"
      >
        {copied ? <Check className="w-3 h-3 text-green-500" /> : <Copy className="w-3 h-3" />}
      </button>
    </div>
  );
}

function ChatSettingsTab() {
  const { terminalShell, setTerminalShell } = useAppStore();
  const {
    skipPermissions,
    setSkipPermissions,
    defaultModel,
    setDefaultModel,
    cliPath,
    setCliPath,
    availableClis,
    detectCli,
    claudeApiKeyOverride,
    claudeBaseUrlOverride,
    codexApiKeyOverride,
    codexBaseUrlOverride,
    setClaudeApiKeyOverride,
    setClaudeBaseUrlOverride,
    setCodexApiKeyOverride,
    setCodexBaseUrlOverride,
  } = useChatStore();

  const isWindows = __IS_TAURI__ && navigator.platform.startsWith("Win");
  const [installMethod, setInstallMethod] = useState<InstallMethod>(
    () => (localStorage.getItem("chat_installMethod") as InstallMethod) || "npm"
  );
  const [detecting, setDetecting] = useState(false);
  const [detected, setDetected] = useState(false);

  const handleDetect = async () => {
    setDetecting(true);
    await detectCli();
    setDetecting(false);
    setDetected(true);
  };

  const handleMethodChange = (m: InstallMethod) => {
    setInstallMethod(m);
    localStorage.setItem("chat_installMethod", m);
    setDetected(false);
  };

  const hint = INSTALL_HINTS[installMethod];
  const claudeNotFound = detected && !availableClis.some((c) => c.cliType === "claude");
  const codexNotFound = detected && !availableClis.some((c) => c.cliType === "codex");

  return (
    <div className="p-4 space-y-4 text-sm">
      <section>
        <h3 className="font-medium mb-2 text-foreground">CLI 状态</h3>
        <div className="space-y-2">
          {availableClis.length > 0 ? (
            availableClis.map((cli, i) => (
              <div
                key={i}
                className="flex items-center gap-2 text-xs text-muted-foreground"
              >
                <div className="w-1.5 h-1.5 bg-green-500 rounded-full shrink-0" />
                <span className="capitalize font-medium">{cli.cliType}</span>
                {cli.version && <span>{cli.version}</span>}
                <span className="truncate text-muted-foreground/60">
                  {cli.path}
                </span>
              </div>
            ))
          ) : (
            <p className="text-xs text-muted-foreground">未检测到已安装的 CLI</p>
          )}

          {/* 安装方式选择 */}
          <div>
            <p className="text-xs text-muted-foreground mb-1.5">安装方式</p>
            <div className="flex flex-wrap gap-1">
              {(["npm", "nvm", "bun", "other"] as InstallMethod[]).map((m) => (
                <button
                  key={m}
                  onClick={() => handleMethodChange(m)}
                  className={`px-2 py-0.5 rounded text-xs border transition-colors ${
                    installMethod === m
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border text-muted-foreground hover:border-foreground/40 hover:text-foreground"
                  }`}
                >
                  {INSTALL_HINTS[m].label}
                </button>
              ))}
            </div>
          </div>

          <button
            onClick={handleDetect}
            disabled={detecting}
            className="flex items-center gap-1 text-xs text-primary hover:text-primary/80 transition-colors disabled:opacity-50"
          >
            <RefreshCw className={`w-3 h-3 ${detecting ? "animate-spin" : ""}`} />
            {detecting ? "检测中..." : "重新检测"}
          </button>

          {/* Claude 检测失败提示 */}
          {claudeNotFound && (
            <div className="rounded-md bg-yellow-500/10 border border-yellow-500/30 p-2.5 space-y-1.5">
              <p className="text-xs font-medium text-yellow-600 dark:text-yellow-400">
                未找到 Claude CLI
              </p>
              <p className="text-[11px] text-muted-foreground">通过 npm 安装：</p>
              <CopyCommandLine cmd="npm install -g @anthropic-ai/claude-code" />
              <ul className="space-y-0.5 mt-1">
                {hint.paths.map((p, i) => (
                  <li key={i} className="text-[11px] text-muted-foreground font-mono">{p}</li>
                ))}
              </ul>
              <p className="text-[11px] text-muted-foreground leading-relaxed">{hint.tip}</p>
            </div>
          )}

          {/* Codex 检测失败提示 */}
          {codexNotFound && (
            <div className="rounded-md bg-blue-500/10 border border-blue-500/30 p-2.5 space-y-1.5">
              <p className="text-xs font-medium text-blue-600 dark:text-blue-400">
                未找到 Codex CLI（可选）
              </p>
              <p className="text-[11px] text-muted-foreground">通过 npm 安装：</p>
              <CopyCommandLine cmd="npm install -g @openai/codex" />
              <p className="text-[11px] text-muted-foreground leading-relaxed">
                安装后点击「重新检测」。若使用 nvm，需先 <code className="font-mono">nvm use</code> 激活对应版本。
              </p>
            </div>
          )}
        </div>
      </section>

      <section>
        <h3 className="font-medium mb-2 text-foreground">CLI 路径</h3>
        <div className="flex gap-1.5">
          <input
            type="text"
            value={cliPath}
            onChange={(e) => setCliPath(e.target.value)}
            placeholder={
              navigator.platform.startsWith("Win")
                ? "C:\\Users\\<user>\\.bun\\bin\\claude.exe"
                : "/usr/local/bin/claude"
            }
            className="flex-1 bg-muted border border-border rounded px-2.5 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary font-mono"
          />
          {__IS_TAURI__ && (
            <button
              onClick={async () => {
                const selected = await openFileDialog({ multiple: false, directory: false });
                if (typeof selected === "string") setCliPath(selected);
              }}
              className="shrink-0 px-2.5 py-1.5 text-xs bg-muted border border-border rounded hover:bg-accent transition-colors"
              title="浏览文件"
            >
              <FolderOpen className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
        <p className="mt-1 text-xs text-muted-foreground">
          留空则自动检测。如自动检测失败，请手动指定 Claude CLI 可执行文件路径
        </p>
      </section>

      <section>
        <h3 className="font-medium mb-2 text-foreground">默认模型</h3>
        <input
          type="text"
          value={defaultModel}
          onChange={(e) => setDefaultModel(e.target.value)}
          placeholder="留空使用 CLI 配置中的默认模型"
          className="w-full bg-muted border border-border rounded px-2.5 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
        />
        <p className="mt-1 text-xs text-muted-foreground">
          新建对话时使用的默认模型（优先于 CLI 配置）
        </p>
      </section>

      {/* Anthropic (Claude) */}
      <section>
        <h3 className="font-medium mb-2 text-foreground">Anthropic (Claude)</h3>
        <CliConfigDisplay />
        <div className="mt-3 space-y-2">
          <div>
            <label className="text-xs text-muted-foreground">手动 API Key（覆盖自动检测）</label>
            <input
              type="password"
              value={claudeApiKeyOverride}
              onChange={(e) => setClaudeApiKeyOverride(e.target.value)}
              placeholder="sk-ant-..."
              className="mt-1 w-full bg-muted border border-border rounded px-2.5 py-1.5 text-xs font-mono text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
            />
          </div>
          <div>
            <label className="text-xs text-muted-foreground">手动 Base URL（覆盖自动检测）</label>
            <input
              type="text"
              value={claudeBaseUrlOverride}
              onChange={(e) => setClaudeBaseUrlOverride(e.target.value)}
              placeholder="https://api.anthropic.com"
              className="mt-1 w-full bg-muted border border-border rounded px-2.5 py-1.5 text-xs font-mono text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
            />
          </div>
        </div>
        <ProviderModelManager source="claude" />
      </section>

      {/* OpenAI (Codex) */}
      <section>
        <h3 className="font-medium mb-2 text-foreground">OpenAI (Codex)</h3>
        <CodexCliConfigDisplay />
        <div className="mt-3 space-y-2">
          <div>
            <label className="text-xs text-muted-foreground">手动 API Key（覆盖自动检测）</label>
            <input
              type="password"
              value={codexApiKeyOverride}
              onChange={(e) => setCodexApiKeyOverride(e.target.value)}
              placeholder="sk-..."
              className="mt-1 w-full bg-muted border border-border rounded px-2.5 py-1.5 text-xs font-mono text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
            />
          </div>
          <div>
            <label className="text-xs text-muted-foreground">手动 Base URL（覆盖自动检测）</label>
            <input
              type="text"
              value={codexBaseUrlOverride}
              onChange={(e) => setCodexBaseUrlOverride(e.target.value)}
              placeholder="https://api.openai.com"
              className="mt-1 w-full bg-muted border border-border rounded px-2.5 py-1.5 text-xs font-mono text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
            />
          </div>
        </div>
        <ProviderModelManager source="codex" />
      </section>

      {isWindows && (
        <section>
          <h3 className="font-medium mb-2 text-foreground">终端类型</h3>
          <div className="flex items-center gap-4">
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="terminalShell"
                value="cmd"
                checked={terminalShell === "cmd"}
                onChange={() => setTerminalShell("cmd")}
                className="rounded-full border-border"
              />
              <span className="text-xs text-foreground">CMD</span>
            </label>
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="terminalShell"
                value="powershell"
                checked={terminalShell === "powershell"}
                onChange={() => setTerminalShell("powershell")}
                className="rounded-full border-border"
              />
              <span className="text-xs text-foreground">PowerShell</span>
            </label>
          </div>
          <p className="mt-1 text-xs text-muted-foreground">
            恢复会话时使用的终端类型
          </p>
        </section>
      )}

      <section>
        <h3 className="font-medium mb-2 text-foreground">权限模式</h3>
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={skipPermissions}
            onChange={(e) => setSkipPermissions(e.target.checked)}
            className="rounded border-border"
          />
          <span className="text-xs text-foreground">
            跳过权限确认 (--dangerously-skip-permissions)
          </span>
        </label>
        <p className="mt-1 text-xs text-yellow-500">
          {skipPermissions
            ? "警告：CLI 将自动执行所有工具操作而不请求确认"
            : "CLI 会在执行文件修改等操作前请求确认"}
        </p>
      </section>
    </div>
  );
}
