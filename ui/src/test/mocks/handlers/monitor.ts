/**
 * MSW request handlers for the Monitor service (port 17003).
 *
 * Provides default responses for all Monitor API endpoints.
 * Override per test with server.use().
 */

import { HttpResponse, http } from "msw";
import {
	makeQueryCatalog,
	makeSystemMetrics,
	makeSystemMetricsHistory,
	makeSystemStatus,
	makeVectorQueryResult,
} from "../factories";

const BASE = "http://localhost:17003";

const DEFAULT_HISTORY = makeSystemMetricsHistory(12);
const DEFAULT_LATEST = DEFAULT_HISTORY[DEFAULT_HISTORY.length - 1];

export const monitorHandlers = [
	// -------------------------------------------------------------------------
	// Health
	// -------------------------------------------------------------------------

	http.get(`${BASE}/health`, () =>
		HttpResponse.json({
			status: "ok",
			service: "agentd-monitor",
			version: "0.2.0",
		}),
	),

	// -------------------------------------------------------------------------
	// Metrics
	// -------------------------------------------------------------------------

	http.get(`${BASE}/metrics`, () => HttpResponse.json(DEFAULT_LATEST)),

	http.get(`${BASE}/history`, () => HttpResponse.json(DEFAULT_HISTORY)),

	http.post(`${BASE}/collect`, () =>
		HttpResponse.json({ metrics: makeSystemMetrics(), alerts: [] }),
	),

	// -------------------------------------------------------------------------
	// Status
	// -------------------------------------------------------------------------

	http.get(`${BASE}/status`, () =>
		HttpResponse.json(
			makeSystemStatus({
				metrics: DEFAULT_LATEST,
				last_collected_at: DEFAULT_LATEST.collected_at,
			}),
		),
	),

	// -------------------------------------------------------------------------
	// Named Prometheus queries
	// -------------------------------------------------------------------------

	http.get(`${BASE}/queries`, () => HttpResponse.json(makeQueryCatalog())),

	http.get(`${BASE}/queries/:name`, ({ params }) =>
		HttpResponse.json(makeVectorQueryResult(String(params.name))),
	),
];
