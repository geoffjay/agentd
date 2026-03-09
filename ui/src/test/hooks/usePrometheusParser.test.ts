/**
 * Tests for the Prometheus text format parser.
 */

import { describe, it, expect } from 'vitest'
import { parsePrometheusText, extractHttpMetrics } from '@/hooks/usePrometheusParser'

// ---------------------------------------------------------------------------
// parsePrometheusText
// ---------------------------------------------------------------------------

describe('parsePrometheusText', () => {
  it('parses a simple metric with no labels', () => {
    const text = 'process_uptime_seconds 42.5\n'
    const metrics = parsePrometheusText(text)
    expect(metrics.samples).toHaveLength(1)
    expect(metrics.samples[0].name).toBe('process_uptime_seconds')
    expect(metrics.samples[0].value).toBe(42.5)
    expect(metrics.samples[0].labels).toEqual({})
  })

  it('parses a metric with labels', () => {
    const text = 'http_requests_total{method="GET",status="200"} 123\n'
    const metrics = parsePrometheusText(text)
    expect(metrics.samples).toHaveLength(1)
    expect(metrics.samples[0].name).toBe('http_requests_total')
    expect(metrics.samples[0].value).toBe(123)
    expect(metrics.samples[0].labels).toEqual({ method: 'GET', status: '200' })
  })

  it('ignores comment lines (# HELP, # TYPE)', () => {
    const text = [
      '# HELP http_requests_total Total HTTP requests',
      '# TYPE http_requests_total counter',
      'http_requests_total 42',
    ].join('\n')
    const metrics = parsePrometheusText(text)
    expect(metrics.samples).toHaveLength(1)
  })

  it('ignores empty lines', () => {
    const text = '\nprocess_uptime_seconds 1\n\n'
    const metrics = parsePrometheusText(text)
    expect(metrics.samples).toHaveLength(1)
  })

  it('parses multiple metrics', () => {
    const text = [
      'http_requests_total{status="200"} 100',
      'http_requests_total{status="500"} 5',
      'http_request_duration_seconds 0.05',
    ].join('\n')
    const metrics = parsePrometheusText(text)
    expect(metrics.samples).toHaveLength(3)
  })

  it('parses optional timestamps', () => {
    const text = 'process_uptime_seconds 42 1700000000000\n'
    const metrics = parsePrometheusText(text)
    expect(metrics.samples[0].timestamp).toBe(1700000000000)
  })

  it('skips lines with unparseable values', () => {
    const text = 'bad_metric notanumber\n'
    const metrics = parsePrometheusText(text)
    expect(metrics.samples).toHaveLength(0)
  })

  describe('.get()', () => {
    it('returns first matching sample', () => {
      const text = [
        'http_requests_total{status="200"} 100',
        'http_requests_total{status="500"} 5',
      ].join('\n')
      const metrics = parsePrometheusText(text)
      const m = metrics.get('http_requests_total')
      expect(m).toBeDefined()
      expect(m?.labels.status).toBe('200')
    })

    it('returns undefined for unknown metric', () => {
      const metrics = parsePrometheusText('process_uptime_seconds 1')
      expect(metrics.get('nonexistent')).toBeUndefined()
    })
  })

  describe('.getAll()', () => {
    it('returns all matching samples', () => {
      const text = [
        'http_requests_total{status="200"} 100',
        'http_requests_total{status="500"} 5',
        'other_metric 1',
      ].join('\n')
      const metrics = parsePrometheusText(text)
      const all = metrics.getAll('http_requests_total')
      expect(all).toHaveLength(2)
    })

    it('returns empty array for unknown metric', () => {
      const metrics = parsePrometheusText('process_uptime_seconds 1')
      expect(metrics.getAll('nonexistent')).toHaveLength(0)
    })
  })

  describe('.sum()', () => {
    it('sums all values for a metric name', () => {
      const text = [
        'http_requests_total{status="200"} 100',
        'http_requests_total{status="500"} 5',
        'http_requests_total{status="404"} 3',
      ].join('\n')
      const metrics = parsePrometheusText(text)
      expect(metrics.sum('http_requests_total')).toBe(108)
    })

    it('returns 0 for unknown metric', () => {
      const metrics = parsePrometheusText('process_uptime_seconds 1')
      expect(metrics.sum('nonexistent')).toBe(0)
    })
  })
})

// ---------------------------------------------------------------------------
// extractHttpMetrics
// ---------------------------------------------------------------------------

describe('extractHttpMetrics', () => {
  it('extracts request totals', () => {
    const text = [
      'http_requests_total{status="200"} 100',
      'http_requests_total{status="201"} 20',
    ].join('\n')
    const metrics = parsePrometheusText(text)
    const http = extractHttpMetrics(metrics)
    expect(http.requestsTotal).toBe(120)
  })

  it('extracts error count from 4xx and 5xx', () => {
    const text = [
      'http_requests_total{status="200"} 100',
      'http_requests_total{status="404"} 10',
      'http_requests_total{status="500"} 5',
    ].join('\n')
    const metrics = parsePrometheusText(text)
    const http = extractHttpMetrics(metrics)
    expect(http.errorsTotal).toBe(15)
  })

  it('calculates error rate', () => {
    const text = [
      'http_requests_total{status="200"} 90',
      'http_requests_total{status="500"} 10',
    ].join('\n')
    const metrics = parsePrometheusText(text)
    const http = extractHttpMetrics(metrics)
    expect(http.errorRate).toBeCloseTo(0.1)
  })

  it('returns 0 error rate when no requests', () => {
    const metrics = parsePrometheusText('process_uptime_seconds 1')
    const http = extractHttpMetrics(metrics)
    expect(http.requestsTotal).toBe(0)
    expect(http.errorRate).toBe(0)
  })

  it('uses status label to identify errors', () => {
    const text = [
      'http_requests_total{status="200"} 80',
      'http_requests_total{code="400"} 20',
    ].join('\n')
    const metrics = parsePrometheusText(text)
    const http = extractHttpMetrics(metrics)
    expect(http.errorsTotal).toBe(20)
  })
})
