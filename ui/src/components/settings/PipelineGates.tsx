/**
 * PipelineGates — read-only reference section for the autonomous pipeline
 * human interaction gates (v0.10.0).
 *
 * Gates define exactly where humans remain in the loop. This section
 * surfaces them visibly in Settings so operators know what the system
 * will and will not do autonomously.
 *
 * Two categories:
 *   Always-human   — hardcoded in the conductor system prompt; cannot be
 *                    changed through the UI or config.
 *   Configurable   — autonomous by default but can be set per-project in
 *                    the agent YAML. The UI shows the default only; actual
 *                    runtime value comes from the conductor YAML.
 *
 * This is intentionally read-only for now. Once the conductor exposes a
 * runtime settings endpoint, the configurable gates can be made interactive.
 */

import { Lock, Settings2 } from 'lucide-react'
import { PIPELINE_GATES } from '@/types/pipeline'
import type { PipelineGate } from '@/types/pipeline'

// ---------------------------------------------------------------------------
// Gate row
// ---------------------------------------------------------------------------

function GateRow({ gate }: { gate: PipelineGate }) {
  const isAlways = gate.kind === 'always'

  return (
    <div className="flex items-start gap-4 py-3">
      {/* Icon */}
      <div
        className={[
          'mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-full',
          isAlways
            ? 'bg-red-50 dark:bg-red-900/20'
            : 'bg-gray-100 dark:bg-gray-700',
        ].join(' ')}
      >
        {isAlways ? (
          <Lock
            size={15}
            aria-hidden="true"
            className="text-red-500 dark:text-red-400"
          />
        ) : (
          <Settings2
            size={15}
            aria-hidden="true"
            className="text-gray-500 dark:text-gray-400"
          />
        )}
      </div>

      {/* Text */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <p className="text-sm font-medium text-gray-900 dark:text-white">
            {gate.label}
          </p>
          <span
            className={[
              'rounded-full px-2 py-0.5 text-xs font-medium',
              isAlways
                ? 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400'
                : gate.defaultAutonomous
                  ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
                  : 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400',
            ].join(' ')}
          >
            {isAlways
              ? 'Always human'
              : gate.defaultAutonomous
                ? 'Autonomous by default'
                : 'Human by default'}
          </span>
        </div>
        <p className="mt-0.5 text-sm text-gray-500 dark:text-gray-400">
          {gate.description}
        </p>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// PipelineGates
// ---------------------------------------------------------------------------

export function PipelineGates() {
  const alwaysGates = PIPELINE_GATES.filter((g) => g.kind === 'always')
  const configurableGates = PIPELINE_GATES.filter((g) => g.kind === 'configurable')

  return (
    <div className="space-y-6">
      {/* Explainer */}
      <p className="text-sm text-gray-500 dark:text-gray-400">
        The autonomous pipeline (v0.10.0) defines explicit gates where human
        approval is required. Always-human gates are hardcoded and cannot be
        overridden. Configurable gates show their default; the actual runtime
        value is set per-project in{' '}
        <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs dark:bg-gray-700">
          .agentd/agents/conductor.yml
        </code>
        .
      </p>

      {/* Always-human gates */}
      <div>
        <h3 className="mb-1 flex items-center gap-2 text-sm font-semibold text-gray-700 dark:text-gray-300">
          <Lock size={13} aria-hidden="true" className="text-red-500" />
          Always requires human
        </h3>
        <div className="divide-y divide-gray-100 rounded-lg border border-gray-200 bg-gray-50 px-4 dark:divide-gray-700 dark:border-gray-700 dark:bg-gray-800/50">
          {alwaysGates.map((gate) => (
            <GateRow key={gate.id} gate={gate} />
          ))}
        </div>
      </div>

      {/* Configurable gates */}
      <div>
        <h3 className="mb-1 flex items-center gap-2 text-sm font-semibold text-gray-700 dark:text-gray-300">
          <Settings2 size={13} aria-hidden="true" className="text-gray-500" />
          Configurable gates
        </h3>
        <div className="divide-y divide-gray-100 rounded-lg border border-gray-200 bg-gray-50 px-4 dark:divide-gray-700 dark:border-gray-700 dark:bg-gray-800/50">
          {configurableGates.map((gate) => (
            <GateRow key={gate.id} gate={gate} />
          ))}
        </div>
        <p className="mt-2 text-xs text-gray-400 dark:text-gray-500">
          To change a configurable gate, update the{' '}
          <code className="font-mono">gates:</code> section in conductor.yml and
          redeploy the agent.
        </p>
      </div>
    </div>
  )
}

export default PipelineGates
