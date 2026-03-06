import { BrowserRouter, Route, Routes } from 'react-router-dom'
import { AppShell } from '@/layouts'
import {
  AgentsPage,
  DashboardPage,
  HooksPage,
  MonitoringPage,
  NotFoundPage,
  NotificationsPage,
  QuestionsPage,
  SettingsPage,
  WorkflowsPage,
} from '@/pages'
import { AgentDetail } from '@/pages/agents/AgentDetail'

function App() {
  return (
    <BrowserRouter>
      <Routes>
        {/* All main pages rendered inside the AppShell layout */}
        <Route element={<AppShell />}>
          <Route index element={<DashboardPage />} />
          <Route path="/agents" element={<AgentsPage />} />
          <Route path="/agents/:id" element={<AgentDetail />} />
          <Route path="/notifications" element={<NotificationsPage />} />
          <Route path="/questions" element={<QuestionsPage />} />
          <Route path="/workflows" element={<WorkflowsPage />} />
          <Route path="/monitoring" element={<MonitoringPage />} />
          <Route path="/hooks" element={<HooksPage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Route>

        {/* 404 catch-all */}
        <Route path="*" element={<NotFoundPage />} />
      </Routes>
    </BrowserRouter>
  )
}

export default App
