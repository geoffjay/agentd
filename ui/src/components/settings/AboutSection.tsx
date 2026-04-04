/**
 * AboutSection — app version, links, and build info.
 */

const APP_VERSION = import.meta.env.VITE_APP_VERSION ?? "0.1.0";

export function AboutSection() {
	return (
		<div className="space-y-4">
			<div className="flex items-center justify-between">
				<span className="text-sm font-medium text-th-text-secondary">
					Version
				</span>
				<span className="text-sm text-th-text-muted">
					{APP_VERSION}
				</span>
			</div>

			<div className="flex items-center justify-between">
				<span className="text-sm font-medium text-th-text-secondary">
					Source
				</span>
				<a
					href="https://github.com/geoffjay/agentd"
					target="_blank"
					rel="noopener noreferrer"
					aria-label="GitHub repository"
					className="text-sm text-th-text-link hover:opacity-80 hover:underline"
				>
					GitHub
				</a>
			</div>

			<div className="flex items-center justify-between">
				<span className="text-sm font-medium text-th-text-secondary">
					Docs
				</span>
				<a
					href="https://github.com/geoffjay/agentd/wiki"
					target="_blank"
					rel="noopener noreferrer"
					aria-label="Documentation"
					className="text-sm text-th-text-link hover:opacity-80 hover:underline"
				>
					Documentation
				</a>
			</div>

			<div className="flex items-center justify-between">
				<span className="text-sm font-medium text-th-text-secondary">
					Built with
				</span>
				<span className="text-sm text-th-text-muted">
					React + Vite + Tailwind CSS
				</span>
			</div>
		</div>
	);
}

export default AboutSection;
