import { Link } from "react-router-dom";

export function NotFoundPage() {
	return (
		<div className="flex flex-col items-center justify-center py-24 text-center">
			<p className="text-6xl font-bold text-th-text-faint">404</p>
			<h1 className="mt-4 text-2xl font-semibold text-th-text">
				Page not found
			</h1>
			<p className="mt-2 text-th-text-muted">
				The page you are looking for does not exist.
			</p>
			<Link
				to="/"
				className="mt-6 rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover"
			>
				Return to dashboard
			</Link>
		</div>
	);
}

export default NotFoundPage;
