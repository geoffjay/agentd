import { BrowserRouter, Route, Routes } from "react-router-dom";
import { ErrorBoundary } from "@/components/common/ErrorBoundary";
import { RequireAuth } from "@/components/common/RequireAuth";
import { AppShell } from "@/layouts";
import {
	AgentsPage,
	ApprovalQueuePage,
	CommunicatePage,
	DashboardPage,
	HooksPage,
	LoginPage,
	MemoriesPage,
	MonitoringPage,
	NotFoundPage,
	NotificationsPage,
	QuestionsPage,
	RegisterPage,
	SettingsPage,
	WorkflowsPage,
} from "@/pages";
import { AgentDetail } from "@/pages/agents/AgentDetail";
import { AgentFormPage } from "@/pages/agents/AgentFormPage";
import { QuestionDetail } from "@/pages/questions/QuestionDetail";
import { WorkflowDetail } from "@/pages/workflows/WorkflowDetail";
import { WorkflowFormPage } from "@/pages/workflows/WorkflowFormPage";

function App() {
	return (
		<ErrorBoundary level="root">
			<BrowserRouter>
				<Routes>
					{/* Public routes — no auth required */}
					<Route path="/login" element={<LoginPage />} />
					<Route path="/register" element={<RegisterPage />} />

					{/* Protected routes — redirect to /login when unauthenticated */}
					<Route element={<RequireAuth />}>
						<Route element={<AppShell />}>
							<Route index element={<DashboardPage />} />
							<Route path="/agents" element={<AgentsPage />} />
							<Route path="/agents/new" element={<AgentFormPage />} />
							<Route path="/agents/:id" element={<AgentDetail />} />
							<Route path="/agents/:id/edit" element={<AgentFormPage />} />
							<Route path="/notifications" element={<NotificationsPage />} />
							<Route path="/questions" element={<QuestionsPage />} />
							<Route path="/questions/:id" element={<QuestionDetail />} />
							<Route path="/workflows" element={<WorkflowsPage />} />
							<Route path="/workflows/new" element={<WorkflowFormPage />} />
							<Route path="/workflows/:id" element={<WorkflowDetail />} />
							<Route
								path="/workflows/:id/edit"
								element={<WorkflowFormPage />}
							/>
							<Route path="/monitoring" element={<MonitoringPage />} />
							<Route path="/hooks" element={<HooksPage />} />
							<Route path="/settings" element={<SettingsPage />} />
							<Route path="/approvals" element={<ApprovalQueuePage />} />
							<Route path="/memories" element={<MemoriesPage />} />
							<Route path="/communicate" element={<CommunicatePage />} />
						</Route>
					</Route>

					{/* 404 catch-all */}
					<Route path="*" element={<NotFoundPage />} />
				</Routes>
			</BrowserRouter>
		</ErrorBoundary>
	);
}

export default App;
