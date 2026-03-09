/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_ASK_SERVICE_URL: string
  readonly VITE_NOTIFY_SERVICE_URL: string
  readonly VITE_ORCHESTRATOR_SERVICE_URL: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
