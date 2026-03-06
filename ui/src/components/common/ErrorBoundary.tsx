/**
 * ErrorBoundary — React class-based error boundary.
 *
 * Catches render errors thrown by child components and shows a graceful
 * fallback instead of a blank / broken screen.
 *
 * Usage:
 *   // Root-level (wraps entire app):
 *   <ErrorBoundary>
 *     <App />
 *   </ErrorBoundary>
 *
 *   // Page-level (wraps a single route):
 *   <ErrorBoundary level="page" onReset={() => navigate('/')}>
 *     <MyPage />
 *   </ErrorBoundary>
 */

import { Component } from 'react'
import type { ErrorInfo, ReactNode } from 'react'
import { AlertTriangle, ChevronDown, ChevronRight, Home, RefreshCw } from 'lucide-react'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ErrorBoundaryProps {
  children: ReactNode
  /** 'root' wraps the whole app; 'page' wraps a single route */
  level?: 'root' | 'page'
  /** Called when the user clicks "Try Again" (useful to reset router state) */
  onReset?: () => void
  /** Custom fallback component (replaces default fallback UI) */
  fallback?: (error: Error, reset: () => void) => ReactNode
}

interface State {
  error: Error | null
  errorInfo: ErrorInfo | null
  detailsOpen: boolean
}

// ---------------------------------------------------------------------------
// Fallback UI
// ---------------------------------------------------------------------------

interface FallbackProps {
  error: Error
  errorInfo: ErrorInfo | null
  level: 'root' | 'page'
  detailsOpen: boolean
  onToggleDetails: () => void
  onReset: () => void
}

function DefaultFallback({
  error,
  errorInfo,
  level,
  detailsOpen,
  onToggleDetails,
  onReset,
}: FallbackProps) {
  return (
    <div
      role="alert"
      className={[
        'flex flex-col items-center justify-center gap-6 p-8 text-center',
        level === 'root' ? 'min-h-screen bg-gray-950' : 'min-h-64 rounded-xl border border-gray-700 bg-gray-800',
      ].join(' ')}
    >
      <div className="flex h-16 w-16 items-center justify-center rounded-full bg-red-900/30">
        <AlertTriangle size={32} className="text-red-400" aria-hidden="true" />
      </div>

      <div className="max-w-md">
        <h2 className="text-xl font-semibold text-white">
          {level === 'root' ? 'Something went wrong' : 'This page crashed'}
        </h2>
        <p className="mt-2 text-sm text-gray-400">
          {level === 'root'
            ? 'An unexpected error occurred. Try refreshing the page.'
            : 'An error occurred while rendering this page. You can try again or go home.'}
        </p>
      </div>

      {/* Action buttons */}
      <div className="flex flex-wrap items-center justify-center gap-3">
        <button
          type="button"
          onClick={onReset}
          className="flex items-center gap-2 rounded-lg bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-500 transition-colors"
        >
          <RefreshCw size={15} aria-hidden="true" />
          Try Again
        </button>

        <a
          href="/"
          className="flex items-center gap-2 rounded-lg border border-gray-600 px-4 py-2 text-sm font-medium text-gray-300 hover:bg-gray-700 transition-colors"
        >
          <Home size={15} aria-hidden="true" />
          Go Home
        </a>
      </div>

      {/* Collapsible error details */}
      <div className="w-full max-w-lg">
        <button
          type="button"
          onClick={onToggleDetails}
          aria-expanded={detailsOpen}
          className="flex w-full items-center justify-center gap-1 text-xs text-gray-500 hover:text-gray-400 transition-colors"
        >
          {detailsOpen ? (
            <ChevronDown size={12} aria-hidden="true" />
          ) : (
            <ChevronRight size={12} aria-hidden="true" />
          )}
          {detailsOpen ? 'Hide' : 'Show'} error details
        </button>

        {detailsOpen && (
          <div className="mt-3 rounded-lg bg-gray-900 p-4 text-left">
            <p className="mb-2 text-xs font-semibold text-red-400">{error.name}: {error.message}</p>
            {error.stack && (
              <pre className="overflow-x-auto whitespace-pre-wrap text-[10px] text-gray-500 font-mono">
                {error.stack}
              </pre>
            )}
            {errorInfo?.componentStack && (
              <>
                <p className="mt-3 mb-1 text-xs font-semibold text-gray-400">Component stack:</p>
                <pre className="overflow-x-auto whitespace-pre-wrap text-[10px] text-gray-500 font-mono">
                  {errorInfo.componentStack}
                </pre>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Error boundary class component
// ---------------------------------------------------------------------------

export class ErrorBoundary extends Component<ErrorBoundaryProps, State> {
  static defaultProps: Partial<ErrorBoundaryProps> = {
    level: 'root',
  }

  constructor(props: ErrorBoundaryProps) {
    super(props)
    this.state = { error: null, errorInfo: null, detailsOpen: false }
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error }
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    console.error('[ErrorBoundary] Uncaught error:', error, errorInfo)
    this.setState({ errorInfo })
  }

  reset = (): void => {
    this.setState({ error: null, errorInfo: null, detailsOpen: false })
    this.props.onReset?.()
  }

  toggleDetails = (): void => {
    this.setState((prev) => ({ detailsOpen: !prev.detailsOpen }))
  }

  render(): ReactNode {
    const { error, errorInfo, detailsOpen } = this.state
    const { children, level = 'root', fallback } = this.props

    if (error) {
      if (fallback) {
        return fallback(error, this.reset)
      }
      return (
        <DefaultFallback
          error={error}
          errorInfo={errorInfo}
          level={level}
          detailsOpen={detailsOpen}
          onToggleDetails={this.toggleDetails}
          onReset={this.reset}
        />
      )
    }

    return children
  }
}

export default ErrorBoundary
