/**
 * Spinner — animated loading indicator.
 *
 * Variants:
 * - 'inline' (default): small spinner for use inside buttons and inline states
 * - 'page': centered full-page spinner for app initialization
 * - 'overlay': centered spinner with semi-transparent backdrop
 */

export type SpinnerSize = 'xs' | 'sm' | 'md' | 'lg'
export type SpinnerVariant = 'inline' | 'page' | 'overlay'

interface SpinnerProps {
  size?: SpinnerSize
  label?: string
  variant?: SpinnerVariant
  className?: string
}

const SIZE_CLASS: Record<SpinnerSize, string> = {
  xs: 'h-3 w-3 border',
  sm: 'h-4 w-4 border-2',
  md: 'h-8 w-8 border-2',
  lg: 'h-12 w-12 border-4',
}

function SpinnerIcon({ size = 'md', className = '' }: { size?: SpinnerSize; className?: string }) {
  return (
    <div
      role="status"
      aria-label="Loading"
      className={[
        'animate-spin rounded-full border-gray-300 border-t-primary-500 dark:border-gray-600 dark:border-t-primary-400',
        SIZE_CLASS[size],
        className,
      ].join(' ')}
    />
  )
}

export function Spinner({ size = 'md', label, variant = 'inline', className = '' }: SpinnerProps) {
  if (variant === 'page') {
    return (
      <div
        className="fixed inset-0 z-50 flex flex-col items-center justify-center gap-4 bg-gray-950"
        role="status"
        aria-live="polite"
        aria-label={label ?? 'Loading application…'}
      >
        <SpinnerIcon size="lg" />
        {label && <p className="text-sm text-gray-400">{label}</p>}
      </div>
    )
  }

  if (variant === 'overlay') {
    return (
      <div
        className="absolute inset-0 z-20 flex items-center justify-center rounded-lg bg-gray-900/60"
        role="status"
        aria-live="polite"
        aria-label={label ?? 'Loading…'}
      >
        <SpinnerIcon size={size} />
        {label && <span className="sr-only">{label}</span>}
      </div>
    )
  }

  // inline (default)
  return (
    <span className={['inline-flex items-center gap-2', className].join(' ')}>
      <SpinnerIcon size={size} />
      {label && <span className="text-sm text-gray-400">{label}</span>}
    </span>
  )
}

export default Spinner
