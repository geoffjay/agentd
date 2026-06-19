import p5 from "p5";
import { useEffect, useRef, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import TOPOLOGY from "vanta/dist/vanta.topology.min";
import { authApi } from "@/services/auth";
import { useAuthStore } from "@/stores/authStore";

export function LoginPage() {
	const [username, setUsername] = useState("");
	const [password, setPassword] = useState("");
	const [error, setError] = useState<string | null>(null);
	const [loading, setLoading] = useState(false);
	const { login } = useAuthStore();
	const navigate = useNavigate();
	const location = useLocation();

	// Animated Vanta "topology" backdrop. Scoped to this component: it is created
	// when the login route mounts and destroyed on unmount, so it never runs on
	// any other page. Vanta TOPOLOGY is a p5-based effect, so we hand it the p5
	// constructor explicitly rather than relying on a global.
	const vantaRef = useRef<HTMLDivElement>(null);
	const vantaEffect = useRef<{ destroy: () => void } | null>(null);

	useEffect(() => {
		if (!vantaRef.current || vantaEffect.current) return;
		vantaEffect.current = TOPOLOGY({
			el: vantaRef.current,
			p5,
			mouseControls: true,
			touchControls: true,
			gyroControls: false,
			minHeight: 200.0,
			minWidth: 200.0,
			scale: 1.0,
			scaleMobile: 1.0,
			color: 0x7f5757,
			backgroundColor: 0x220000,
		});
		return () => {
			vantaEffect.current?.destroy();
			vantaEffect.current = null;
		};
	}, []);
	const from =
		(location.state as { from?: { pathname?: string } } | null)?.from
			?.pathname ?? "/";

	const handleSubmit = async (e: React.FormEvent) => {
		e.preventDefault();
		setLoading(true);
		setError(null);
		try {
			const resp = await authApi.login({ username, password });
			login(resp.token, resp.user);
			navigate(from, { replace: true });
		} catch (err) {
			setError(err instanceof Error ? err.message : "Login failed");
		} finally {
			setLoading(false);
		}
	};

	return (
		<div className="relative flex min-h-screen items-center justify-center overflow-hidden bg-th-bg px-4">
			<div ref={vantaRef} aria-hidden="true" className="absolute inset-0 z-0" />
			<div className="relative z-10 w-full max-w-sm space-y-6 rounded-xl border border-th-border bg-th-bg/80 p-8 shadow-2xl backdrop-blur-sm">
				<h2 className="text-center text-2xl font-semibold text-th-text">
					Sign in to agentd
				</h2>

				{error && (
					<p className="rounded border border-red-400 bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-900/20 dark:text-red-400">
						{error}
					</p>
				)}

				<form onSubmit={handleSubmit} className="space-y-4">
					<div className="space-y-1">
						<label
							htmlFor="login-username"
							className="block text-sm font-medium text-th-text-secondary"
						>
							Username
						</label>
						<input
							id="login-username"
							className="w-full rounded border border-th-border bg-th-bg-secondary px-3 py-2 text-sm text-th-text placeholder-th-text-muted focus:outline-none focus:ring-2 focus:ring-th-accent"
							value={username}
							onChange={(e) => setUsername(e.target.value)}
							required
						/>
					</div>

					<div className="space-y-1">
						<label
							htmlFor="login-password"
							className="block text-sm font-medium text-th-text-secondary"
						>
							Password
						</label>
						<input
							id="login-password"
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
						{loading ? "Signing in..." : "Sign in"}
					</button>
				</form>

				<p className="text-center text-sm text-th-text-secondary">
					Don&apos;t have an account?{" "}
					<Link to="/register" className="text-th-accent hover:underline">
						Register
					</Link>
				</p>
			</div>
		</div>
	);
}
