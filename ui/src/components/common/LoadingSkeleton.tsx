/**
 * LoadingSkeleton — animated placeholder blocks for loading states.
 */

interface SkeletonProps {
  className?: string
}

/** Single animated skeleton block */
export function Skeleton({ className = '' }: SkeletonProps) {
  return (
    <div
      aria-hidden="true"
      className={['animate-pulse rounded bg-gray-200 dark:bg-gray-700', className].join(' ')}
    />
  )
}

/** Card-shaped skeleton */
export function CardSkeleton() {
  return (
    <div
      role="status"
      aria-busy="true"
      aria-label="Loading…"
      className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
    >
      <div className="flex items-center gap-3">
        <Skeleton className="h-10 w-10 rounded-full" />
        <div className="flex-1 space-y-2">
          <Skeleton className="h-4 w-1/3" />
          <Skeleton className="h-3 w-1/2" />
        </div>
      </div>
      <div className="mt-4 space-y-2">
        <Skeleton className="h-3 w-full" />
        <Skeleton className="h-3 w-4/5" />
      </div>
    </div>
  )
}

/** List-item skeleton */
export function ListItemSkeleton({ rows = 3 }: { rows?: number }) {
  return (
    <div aria-busy="true" aria-label="Loading…" className="space-y-3">
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="flex items-center gap-3">
          <Skeleton className="h-8 w-8 rounded-full" />
          <div className="flex-1 space-y-1.5">
            <Skeleton className="h-3 w-2/3" />
            <Skeleton className="h-3 w-1/2" />
          </div>
          <Skeleton className="h-5 w-14 rounded-full" />
        </div>
      ))}
    </div>
  )
}

/** Chart placeholder skeleton */
export function ChartSkeleton({ height = 160 }: { height?: number }) {
  return (
    <div aria-busy="true" aria-label="Loading chart…" style={{ height }}>
      <Skeleton className="h-full w-full" />
    </div>
  )
}

export default Skeleton
