/**
 * Barrel export for all test data factories.
 *
 * Import factories like:
 *   import { makeAgent, makeNotification } from '@/test/mocks/factories'
 */

export {
  makeAgent,
  makeAgentList,
  makeAgentConfig,
  makePendingApproval,
  makeApprovalList,
  resetAgentSeq,
} from './agent'

export {
  makeNotification,
  makeUrgentNotification,
  makeNotificationList,
  makeCountResponse,
  makeStatusCount,
  resetNotificationSeq,
} from './notification'

export {
  makeQuestionInfo,
  makeTriggerResponse,
  makeAnswerResponse,
  resetQuestionSeq,
} from './question'
