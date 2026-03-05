import { Link } from 'react-router-dom'

export function NotFoundPage() {
  return (
    <div className="flex flex-col items-center justify-center py-24 text-center">
      <p className="text-6xl font-bold text-gray-200 dark:text-gray-700">404</p>
      <h1 className="mt-4 text-2xl font-semibold text-gray-900 dark:text-white">
        Page not found
      </h1>
      <p className="mt-2 text-gray-500 dark:text-gray-400">
        The page you are looking for does not exist.
      </p>
      <Link
        to="/"
        className="mt-6 rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-700"
      >
        Return to dashboard
      </Link>
    </div>
  )
}

export default NotFoundPage
