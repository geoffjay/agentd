import { BrowserRouter, Route, Routes } from "react-router-dom";
import { ErrorBoundary } from "@/components/common/ErrorBoundary";
import { AppShell } from "@/layouts";
import {
	AgentsPage,
	ApprovalQueuePage,
	CommunicatePage,
	DashboardPage,
	HooksPage,
	IndexPage,
	MemoriesPage,
	MonitoringPage,
	NotFoundPage,
	NotificationsPage,
	QuestionsPage,
	SettingsPage,
	WorkflowsPage,
} from "@/pages";
import { AgentDetail } from "@/pages/agents/AgentDetail";
import { QuestionDetail } from "@/pages/questions/QuestionDetail";
import { WorkflowBuilder } from "@/pages/workflows/WorkflowBuilder";
import { WorkflowDetail } from "@/pages/workflows/WorkflowDetail";

function App() {
	return (
		<ErrorBoundary level="root">
			<BrowserRouter>
				<Routes>
					{/* All main pages rendered inside the AppShell layout */}
					<Route element={<AppShell />}>
						<Route index element={<DashboardPage />} />
						<Route path="/agents" element={<AgentsPage />} />
						<Route path="/agents/:id" element={<AgentDetail />} />
						<Route path="/notifications" element={<NotificationsPage />} />
						<Route path="/questions" element={<QuestionsPage />} />
							<Route path="/questions/:id" element={<QuestionDetail />} />
						<Route path="/workflows" element={<WorkflowsPage />} />
						<Route path="/workflows/builder" element={<WorkflowBuilder />} />
						<Route path="/workflows/:id" element={<WorkflowDetail />} />
						<Route path="/workflows/:id/edit" element={<WorkflowBuilder />} />
						<Route path="/monitoring" element={<MonitoringPage />} />
						<Route path="/hooks" element={<HooksPage />} />
						<Route path="/settings" element={<SettingsPage />} />
						<Route path="/approvals" element={<ApprovalQueuePage />} />
						<Route path="/memories" element={<MemoriesPage />} />
						<Route path="/code-index" element={<IndexPage />} />
						<Route path="/communicate" element={<CommunicatePage />} />
					</Route>

					{/* 404 catch-all */}
					<Route path="*" element={<NotFoundPage />} />
				</Routes>
			</BrowserRouter>
		</ErrorBoundary>
	);
}

export default App;
