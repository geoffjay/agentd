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

import { orchestratorHandlers } from './orchestrator'
import { notifyHandlers } from './notify'
import { askHandlers } from './ask'

export const handlers = [...orchestratorHandlers, ...notifyHandlers, ...askHandlers]

export { orchestratorHandlers, notifyHandlers, askHandlers }
