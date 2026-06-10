/**
 * Combined MSW handlers for all agentd services.
 *
 * Import these in the MSW server setup and in individual tests
 * when you need to override specific endpoints:
 *
 *   server.use(
 *     http.get('http://localhost:17006/agents', () =>
 *       HttpResponse.json(paginated([]))
 *     )
 *   )
 */

import { askHandlers } from "./ask";
import { communicateHandlers } from "./communicate";
import { memoryHandlers } from "./memory";
import { monitorHandlers } from "./monitor";
import { notifyHandlers } from "./notify";
import { orchestratorHandlers } from "./orchestrator";

export const handlers = [
	...orchestratorHandlers,
	...notifyHandlers,
	...askHandlers,
	...memoryHandlers,
	...communicateHandlers,
	...monitorHandlers,
];

export {
	askHandlers,
	communicateHandlers,
	memoryHandlers,
	monitorHandlers,
	notifyHandlers,
	orchestratorHandlers,
};
