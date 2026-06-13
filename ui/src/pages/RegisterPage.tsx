import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { useAuthStore } from "@/stores/authStore";
import { authApi } from "@/services/auth";

export function RegisterPage() {
	const [username, setUsername] = useState("");
	const [email, setEmail] = useState("");
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
			const resp = await authApi.register({ username, email, password });
			login(resp.token);
			navigate("/");
		} catch (err) {
			setError(err instanceof Error ? err.message : "Registration failed");
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
				<h2>Create an agentd account</h2>
				{error && <p style={{ color: "red" }}>{error}</p>}
				<form onSubmit={handleSubmit}>
					<div style={{ marginBottom: 12 }}>
						<label htmlFor="register-username" style={{ display: "block" }}>
							Username
						</label>
						<input
							id="register-username"
							value={username}
							onChange={(e) => setUsername(e.target.value)}
							required
							style={{ width: "100%" }}
						/>
					</div>
					<div style={{ marginBottom: 12 }}>
						<label htmlFor="register-email" style={{ display: "block" }}>
							Email
						</label>
						<input
							id="register-email"
							type="email"
							value={email}
							onChange={(e) => setEmail(e.target.value)}
							required
							style={{ width: "100%" }}
						/>
					</div>
					<div style={{ marginBottom: 16 }}>
						<label htmlFor="register-password" style={{ display: "block" }}>
							Password
						</label>
						<input
							id="register-password"
							type="password"
							value={password}
							onChange={(e) => setPassword(e.target.value)}
							required
							style={{ width: "100%" }}
						/>
					</div>
					<button type="submit" disabled={loading} style={{ width: "100%" }}>
						{loading ? "Creating account..." : "Create account"}
					</button>
				</form>
				<p style={{ marginTop: 16 }}>
					Already have an account? <Link to="/login">Sign in</Link>
				</p>
			</div>
		</div>
	);
}
