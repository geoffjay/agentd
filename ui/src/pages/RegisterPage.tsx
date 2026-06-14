import { useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { authApi } from "@/services/auth";
import { useAuthStore } from "@/stores/authStore";

export function RegisterPage() {
	const [username, setUsername] = useState("");
	const [email, setEmail] = useState("");
	const [password, setPassword] = useState("");
	const [error, setError] = useState<string | null>(null);
	const [loading, setLoading] = useState(false);
	const { login } = useAuthStore();
	const navigate = useNavigate();
	const location = useLocation();
	const from =
		(location.state as { from?: { pathname?: string } } | null)?.from
			?.pathname ?? "/";

	const handleSubmit = async (e: React.FormEvent) => {
		e.preventDefault();
		setLoading(true);
		setError(null);
		try {
			const resp = await authApi.register({ username, email, password });
			login(resp.token, resp.user);
			navigate(from, { replace: true });
		} catch (err) {
			setError(err instanceof Error ? err.message : "Registration failed");
		} finally {
			setLoading(false);
		}
	};

	return (
		<div className="flex min-h-screen items-center justify-center bg-th-bg px-4">
			<div className="w-full max-w-sm space-y-6">
				<h2 className="text-center text-2xl font-semibold text-th-text">
					Create an agentd account
				</h2>

				{error && (
					<p className="rounded border border-red-400 bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-900/20 dark:text-red-400">
						{error}
					</p>
				)}

				<form onSubmit={handleSubmit} className="space-y-4">
					<div className="space-y-1">
						<label
							htmlFor="register-username"
							className="block text-sm font-medium text-th-text-secondary"
						>
							Username
						</label>
						<input
							id="register-username"
							className="w-full rounded border border-th-border bg-th-bg-secondary px-3 py-2 text-sm text-th-text placeholder-th-text-muted focus:outline-none focus:ring-2 focus:ring-th-accent"
							value={username}
							onChange={(e) => setUsername(e.target.value)}
							required
						/>
					</div>

					<div className="space-y-1">
						<label
							htmlFor="register-email"
							className="block text-sm font-medium text-th-text-secondary"
						>
							Email
						</label>
						<input
							id="register-email"
							type="email"
							className="w-full rounded border border-th-border bg-th-bg-secondary px-3 py-2 text-sm text-th-text placeholder-th-text-muted focus:outline-none focus:ring-2 focus:ring-th-accent"
							value={email}
							onChange={(e) => setEmail(e.target.value)}
							required
						/>
					</div>

					<div className="space-y-1">
						<label
							htmlFor="register-password"
							className="block text-sm font-medium text-th-text-secondary"
						>
							Password
						</label>
						<input
							id="register-password"
							type="password"
							className="w-full rounded border border-th-border bg-th-bg-secondary px-3 py-2 text-sm text-th-text placeholder-th-text-muted focus:outline-none focus:ring-2 focus:ring-th-accent"
							value={password}
							onChange={(e) => setPassword(e.target.value)}
							required
						/>
					</div>

					<button
						type="submit"
						disabled={loading}
						className="w-full rounded bg-th-accent px-4 py-2 text-sm font-medium text-white hover:bg-th-accent-hover disabled:opacity-50"
					>
						{loading ? "Creating account..." : "Create account"}
					</button>
				</form>

				<p className="text-center text-sm text-th-text-secondary">
					Already have an account?{" "}
					<Link to="/login" className="text-th-accent hover:underline">
						Sign in
					</Link>
				</p>
			</div>
		</div>
	);
}
