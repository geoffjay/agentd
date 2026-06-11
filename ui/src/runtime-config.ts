/**
 * Runtime service configuration fetched from the hosting agentd-ui service.
 *
 * The prebuilt SPA cannot know per-host service locations at build time, so
 * `main.tsx` fetches `/config.json` (served by agentd-ui from the shared
 * agentd config file) before the application module graph loads. Service
 * URLs resolve as: explicit `url` override from the config file, otherwise
 * the page's own protocol and hostname combined with the service's port.
 *
 * When the endpoint is unavailable (e.g. the Vite dev server) the app falls
 * back to the build-time `VITE_*` env defaults in `services/config.ts`.
 */

export interface RuntimeServiceEntry {
	port: number;
	url?: string;
}

export interface RuntimeConfig {
	version: string;
	services: Record<string, RuntimeServiceEntry>;
}

let runtimeConfig: RuntimeConfig | null = null;

export function setRuntimeConfig(config: RuntimeConfig | null): void {
	runtimeConfig = config;
}

export function getRuntimeConfig(): RuntimeConfig | null {
	return runtimeConfig;
}

function isRuntimeConfig(value: unknown): value is RuntimeConfig {
	if (typeof value !== "object" || value === null) return false;
	const services = (value as { services?: unknown }).services;
	if (typeof services !== "object" || services === null) return false;
	return Object.values(services).every(
		(entry) =>
			typeof entry === "object" &&
			entry !== null &&
			typeof (entry as RuntimeServiceEntry).port === "number",
	);
}

/**
 * Fetch `/config.json` from the origin serving the SPA.
 *
 * Returns `null` when the endpoint is missing, returns non-JSON (an SPA
 * index.html fallback), or fails validation — callers treat that as "no
 * runtime config" and build-time defaults apply.
 */
export async function loadRuntimeConfig(): Promise<RuntimeConfig | null> {
	try {
		const response = await fetch("/config.json", {
			headers: { accept: "application/json" },
		});
		if (!response.ok) return null;
		const contentType = response.headers.get("content-type") ?? "";
		if (!contentType.includes("application/json")) return null;
		const data: unknown = await response.json();
		return isRuntimeConfig(data) ? data : null;
	} catch {
		return null;
	}
}

/**
 * Resolve the browser-facing URL for a service from the runtime config, or
 * `undefined` when no runtime config is loaded or the service is unknown.
 */
export function runtimeServiceUrl(name: string): string | undefined {
	const entry = runtimeConfig?.services[name];
	if (!entry) return undefined;
	if (entry.url) return entry.url;
	return `${window.location.protocol}//${window.location.hostname}:${entry.port}`;
}
