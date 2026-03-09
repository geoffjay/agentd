/**
 * AboutSection — app version, links, and build info.
 */

const APP_VERSION = import.meta.env.VITE_APP_VERSION ?? '0.1.0'

export function AboutSection() {
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Version</span>
        <span className="text-sm text-gray-600 dark:text-gray-400">{APP_VERSION}</span>
      </div>

      <div className="flex items-center justify-between">
        <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Source</span>
        <a
          href="https://github.com/geoffjay/agentd"
          target="_blank"
          rel="noopener noreferrer"
          aria-label="GitHub repository"
          className="text-sm text-primary-600 hover:text-primary-700 hover:underline dark:text-primary-400 dark:hover:text-primary-300"
        >
          GitHub
        </a>
      </div>

      <div className="flex items-center justify-between">
        <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Docs</span>
        <a
          href="https://github.com/geoffjay/agentd/wiki"
          target="_blank"
          rel="noopener noreferrer"
          aria-label="Documentation"
          className="text-sm text-primary-600 hover:text-primary-700 hover:underline dark:text-primary-400 dark:hover:text-primary-300"
        >
          Documentation
        </a>
      </div>

      <div className="flex items-center justify-between">
        <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Built with</span>
        <span className="text-sm text-gray-600 dark:text-gray-400">
          React + Vite + Tailwind CSS
        </span>
      </div>
    </div>
  )
}

export default AboutSection
