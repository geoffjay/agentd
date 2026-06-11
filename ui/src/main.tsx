import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import { loadRuntimeConfig, setRuntimeConfig } from "./runtime-config";

// Resolve runtime service configuration before the application module graph
// loads: API clients capture their base URLs at module evaluation time, so
// `App` must be imported only after the config is in place.
loadRuntimeConfig().then(async (config) => {
	setRuntimeConfig(config);
	const { default: App } = await import("./App.tsx");
	const root = document.getElementById("root");
	if (!root) throw new Error("missing #root element");
	createRoot(root).render(
		<StrictMode>
			<App />
		</StrictMode>,
	);
});
