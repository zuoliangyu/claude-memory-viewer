import { Routes, Route, Navigate } from "react-router-dom";
import { AppLayout } from "./components/layout/AppLayout";
import { ProjectsPage } from "./components/project/ProjectsPage";
import { SessionsPage } from "./components/session/SessionsPage";
import { MessagesPage } from "./components/message/MessagesPage";
import { SearchPage } from "./components/search/SearchPage";
import { StatsPage } from "./components/stats/StatsPage";
import { ChatPage } from "./components/chat/ChatPage";
import { QuickChatPage } from "./components/quick-chat/QuickChatPage";

function App() {
  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route path="/" element={<Navigate to="/projects" replace />} />
        <Route path="/projects" element={<ProjectsPage />} />
        <Route path="/projects/:projectId" element={<SessionsPage />} />
        <Route
          path="/projects/:projectId/session/*"
          element={<MessagesPage />}
        />
        <Route path="/search" element={<SearchPage />} />
        <Route path="/stats" element={<StatsPage />} />
        <Route path="/chat" element={<ChatPage />} />
        <Route path="/chat/:sessionId" element={<ChatPage />} />
        <Route path="/quick-chat" element={<QuickChatPage />} />
      </Route>
    </Routes>
  );
}

export default App;
