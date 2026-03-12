import { defineConfig, loadEnv } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '')

  const askServiceUrl = env.VITE_AGENTD_ASK_SERVICE_URL || 'http://localhost:17001'
  const notifyServiceUrl = env.VITE_AGENTD_NOTIFY_SERVICE_URL || 'http://localhost:17004'
  const orchestratorServiceUrl = env.VITE_AGENTD_ORCHESTRATOR_SERVICE_URL || 'http://localhost:17006'

  return {
    plugins: [react(), tailwindcss()],
    resolve: {
      alias: {
        '@': path.resolve(__dirname, './src'),
        '@/components': path.resolve(__dirname, './src/components'),
        '@/hooks': path.resolve(__dirname, './src/hooks'),
        '@/layouts': path.resolve(__dirname, './src/layouts'),
        '@/pages': path.resolve(__dirname, './src/pages'),
        '@/services': path.resolve(__dirname, './src/services'),
        '@/types': path.resolve(__dirname, './src/types'),
        '@/utils': path.resolve(__dirname, './src/utils'),
        '@/stores': path.resolve(__dirname, './src/stores'),
        '@/test': path.resolve(__dirname, './src/test'),
        '@/styles': path.resolve(__dirname, './src/styles'),
      },
    },
    server: {
      port: 3000,
      proxy: {
        '/api/ask': {
          target: askServiceUrl,
          changeOrigin: true,
          rewrite: (path) => path.replace(/^\/api\/ask/, ''),
        },
        '/api/notify': {
          target: notifyServiceUrl,
          changeOrigin: true,
          rewrite: (path) => path.replace(/^\/api\/notify/, ''),
        },
        '/api/orchestrator': {
          target: orchestratorServiceUrl,
          changeOrigin: true,
          rewrite: (path) => path.replace(/^\/api\/orchestrator/, ''),
        },
      },
      watch: {
        ignored: ['design/**/*'],
      },
    },
    test: {
      globals: true,
      environment: 'jsdom',
      setupFiles: ['./src/test/setup.ts'],
      coverage: {
        provider: 'v8',
        reporter: ['text', 'json', 'html'],
        // Exclude test infrastructure, config, and generated files
        exclude: [
          'src/test/**',
          'src/main.tsx',
          'src/types/env.d.ts',
          '**/*.config.*',
          '**/index.ts',
        ],
        thresholds: {
          lines: 60,
          functions: 60,
          branches: 50,
          statements: 60,
        },
      },
    },
  }
})
