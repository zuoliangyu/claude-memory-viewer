import { useEffect, useState } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { useAppStore } from "../../stores/appStore";
import { useTheme } from "../../hooks/useTheme";
import { useUpdateChecker } from "../../hooks/useUpdateChecker";
import { useFileWatcher } from "../../hooks/useFileWatcher";
import { UpdateIndicator } from "./UpdateIndicator";
import {
  FolderOpen,
  Search,
  BarChart3,
  ChevronRight,
  Bot,
  Terminal,
  Sun,
  Moon,
  Monitor,
  X,
  Mail,
  Users,
  Github,
  ExternalLink,
} from "lucide-react";

declare const __IS_TAURI__: boolean;
declare const __APP_VERSION__: string;

export function Sidebar() {
  const navigate = useNavigate();
  const location = useLocation();
  const { source, setSource, projects, loadProjects, projectsLoading } =
    useAppStore();
  const { theme, setTheme } = useTheme();
  const [showAbout, setShowAbout] = useState(false);
  useUpdateChecker();
  useFileWatcher();

  useEffect(() => {
    loadProjects();
  }, [source]);

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

  const handleSourceChange = (s: "claude" | "codex") => {
    if (s !== source) {
      setSource(s);
      navigate("/projects");
    }
  };

  return (
    <aside className="w-64 h-full border-r border-border bg-card flex flex-col shrink-0">
      {/* Header */}
      <div className="p-4 border-b border-border">
        <h1 className="text-sm font-semibold text-foreground mb-3">
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
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 overflow-y-auto p-2">
        {/* Quick links */}
        <div className="mb-4">
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
        </div>

        {/* Projects list */}
        <div>
          <h2 className="px-3 py-1 text-xs font-medium text-muted-foreground uppercase tracking-wider">
            项目 ({projects.length})
          </h2>
          {projectsLoading ? (
            <div className="px-3 py-2 text-sm text-muted-foreground">
              加载中...
            </div>
          ) : (
            <div className="mt-1 space-y-0.5">
              {projects.map((project) => (
                <button
                  key={project.id}
                  onClick={() =>
                    navigate(
                      `/projects/${encodeURIComponent(project.id)}`
                    )
                  }
                  className={`w-full flex items-center gap-2 px-3 py-1.5 rounded-md text-sm transition-colors group ${
                    isProjectActive(project.id)
                      ? "bg-accent text-accent-foreground"
                      : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                  }`}
                  title={project.displayPath}
                >
                  <FolderOpen className="w-3.5 h-3.5 shrink-0" />
                  <span className="truncate flex-1 text-left">
                    {project.shortName}
                  </span>
                  <span className="text-xs text-muted-foreground shrink-0">
                    {project.sessionCount}
                  </span>
                  <ChevronRight className="w-3 h-3 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity" />
                </button>
              ))}
            </div>
          )}
        </div>
      </nav>

      {/* Footer */}
      <div className="p-3 border-t border-border space-y-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">
              {projects.length} 个项目
            </span>
            <button
              onClick={() => setShowAbout(true)}
              className="px-1.5 py-0.5 rounded border border-border text-xs text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
            >
              关于作者
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
        {__IS_TAURI__ && <UpdateIndicator />}
      </div>

      {/* About Modal */}
      {showAbout && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
          onClick={() => setShowAbout(false)}
        >
          <div
            className="bg-card border border-border rounded-lg shadow-lg w-80 max-w-[90vw]"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Modal Header */}
            <div className="flex items-center justify-between p-4 border-b border-border">
              <h2 className="text-sm font-semibold text-foreground">关于作者</h2>
              <button
                onClick={() => setShowAbout(false)}
                className="p-1 rounded transition-colors text-muted-foreground hover:text-foreground hover:bg-accent/50"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            {/* Modal Body */}
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
          </div>
        </div>
      )}
    </aside>
  );
}
