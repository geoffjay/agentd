/**
 * ContentArea — scrollable main content wrapper.
 *
 * Offsets for the fixed header (h-16 = 64px) and sidebar width.
 * Transitions smoothly when the sidebar expands/collapses.
 */

import type { ReactNode } from "react";
import { useLayout } from "./context";

interface ContentAreaProps {
	children: ReactNode;
}

export function ContentArea({ children }: ContentAreaProps) {
	const { sidebarOpen: _sidebarOpen } = useLayout();

	return (
		<main
			id="main-content"
			className={[
				"min-h-[calc(100vh-4rem)]",
				"mt-12", // offset for fixed header
				"overflow-y-auto",
				"transition-all duration-300 ease-in-out",
			].join(" ")}
		>
			{/* Inner wrapper: responsive padding + max-width centering */}
			<div className="mx-auto max-w-screen-2xl p-4 md:p-6 lg:p-8">
				{children}
			</div>
		</main>
	);
}

export default ContentArea;
