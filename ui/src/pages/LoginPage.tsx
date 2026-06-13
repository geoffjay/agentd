import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { useAuthStore } from "@/stores/authStore";
import { authApi } from "@/services/auth";

export function LoginPage() {
	const [username, setUsername] = useState("");
	const [password, setPassword] = useState("");
	const [error, setError] = useState<string | null>(null);
	const [loading, setLoading] = useState(false);
	const { login } = useAuthStore();
	const navigate = useNavigate();

	const handleSubmit = async (e: React.FormEvent) => {
		e.preventDefault();
		setLoading(true);
		setError(null);
		try {
			const resp = await authApi.login({ username, password });
			login(resp.token);
			navigate("/");
		} catch (err) {
			setError(err instanceof Error ? err.message : "Login failed");
		} finally {
			setLoading(false);
		}
	};

	return (
		<div
			style={{
				display: "flex",
				justifyContent: "center",
				alignItems: "center",
				height: "100vh",
			}}
		>
			<div style={{ width: 320 }}>
				<h2>Sign in to agentd</h2>
				{error && <p style={{ color: "red" }}>{error}</p>}
				<form onSubmit={handleSubmit}>
					<div style={{ marginBottom: 12 }}>
						<label htmlFor="login-username" style={{ display: "block" }}>
							Username
						</label>
						<input
							id="login-username"
							value={username}
							onChange={(e) => setUsername(e.target.value)}
							required
							style={{ width: "100%" }}
						/>
					</div>
					<div style={{ marginBottom: 16 }}>
						<label htmlFor="login-password" style={{ display: "block" }}>
							Password
						</label>
						<input
							id="login-password"
							type="password"
							value={password}
							onChange={(e) => setPassword(e.target.value)}
							required
							style={{ width: "100%" }}
						/>
					</div>
					<button type="submit" disabled={loading} style={{ width: "100%" }}>
						{loading ? "Signing in..." : "Sign in"}
					</button>
				</form>
				<p style={{ marginTop: 16 }}>
					Don&apos;t have an account? <Link to="/register">Register</Link>
				</p>
			</div>
		</div>
	);
}
