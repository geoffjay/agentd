/**
 * usePrometheusParser — utilities for parsing Prometheus text format metrics.
 *
 * Prometheus text format lines look like:
 *   # HELP metric_name Description of metric.
 *   # TYPE metric_name counter
 *   metric_name{label="value"} 42.0 1234567890000
 *   metric_name 7
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PrometheusMetric {
  name: string
  labels: Record<string, string>
  value: number
  timestamp?: number
}

export interface ParsedMetrics {
  /** All individual metric samples */
  samples: PrometheusMetric[]
  /** Look up a metric by name (returns first match) */
  get: (name: string) => PrometheusMetric | undefined
  /** Look up all samples for a metric name */
  getAll: (name: string) => PrometheusMetric[]
  /** Sum all values for a given metric name */
  sum: (name: string) => number
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/**
 * Parse a Prometheus text exposition format string into structured samples.
 *
 * Handles:
 * - Comment lines (# HELP / # TYPE)
 * - Label sets ({key="value", ...})
 * - Float/NaN/+Inf/-Inf values
 * - Optional timestamps
 */
export function parsePrometheusText(text: string): ParsedMetrics {
  const samples: PrometheusMetric[] = []

  for (const rawLine of text.split('\n')) {
    const line = rawLine.trim()
    if (!line || line.startsWith('#')) continue

    // Match: name{labels} value [timestamp]
    // or:   name value [timestamp]
    const braceIdx = line.indexOf('{')
    const spaceIdx = line.indexOf(' ')

    let name: string
    let labelsStr: string = ''
    let rest: string

    if (braceIdx !== -1 && braceIdx < spaceIdx) {
      name = line.slice(0, braceIdx)
      const closeBrace = line.indexOf('}', braceIdx)
      labelsStr = line.slice(braceIdx + 1, closeBrace)
      rest = line.slice(closeBrace + 1).trim()
    } else {
      name = line.slice(0, spaceIdx)
      rest = line.slice(spaceIdx + 1).trim()
    }

    if (!name) continue

    const parts = rest.split(/\s+/)
    const rawValue = parts[0]
    const timestamp = parts[1] ? parseInt(parts[1], 10) : undefined

    const value = parseFloat(rawValue)
    if (isNaN(value) && rawValue !== 'NaN') continue // skip unparseable

    const labels = parseLabels(labelsStr)
    samples.push({ name, labels, value: isNaN(value) ? 0 : value, timestamp })
  }

  return makeParsedMetrics(samples)
}

function parseLabels(labelsStr: string): Record<string, string> {
  if (!labelsStr.trim()) return {}
  const labels: Record<string, string> = {}
  // Match key="value" pairs
  const re = /(\w+)="([^"]*)"/g
  let m
  while ((m = re.exec(labelsStr)) !== null) {
    labels[m[1]] = m[2]
  }
  return labels
}

function makeParsedMetrics(samples: PrometheusMetric[]): ParsedMetrics {
  return {
    samples,
    get: (name) => samples.find((s) => s.name === name),
    getAll: (name) => samples.filter((s) => s.name === name),
    sum: (name) =>
      samples.filter((s) => s.name === name).reduce((acc, s) => acc + s.value, 0),
  }
}

// ---------------------------------------------------------------------------
// Convenience: extract common HTTP metrics
// ---------------------------------------------------------------------------

export interface HttpMetricsSummary {
  /** Total requests (sum of http_requests_total) */
  requestsTotal: number
  /** Total errors (5xx + 4xx) */
  errorsTotal: number
  /** Error rate 0–1 */
  errorRate: number
  /** P50/P95/P99 latency ms (if histogram available) */
  latencyP50?: number
  latencyP99?: number
}

export function extractHttpMetrics(metrics: ParsedMetrics): HttpMetricsSummary {
  const requestsTotal = metrics.sum('http_requests_total')

  // Count errors: status codes 4xx and 5xx
  const errorSamples = metrics
    .getAll('http_requests_total')
    .filter((s) => {
      const code = parseInt(s.labels.status ?? s.labels.code ?? '0', 10)
      return code >= 400
    })
  const errorsTotal = errorSamples.reduce((acc, s) => acc + s.value, 0)
  const errorRate = requestsTotal > 0 ? errorsTotal / requestsTotal : 0

  return { requestsTotal, errorsTotal, errorRate }
}
