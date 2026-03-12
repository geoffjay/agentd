/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_AGENTD_ASK_SERVICE_URL: string
  readonly VITE_AGENTD_NOTIFY_SERVICE_URL: string
  readonly VITE_AGENTD_ORCHESTRATOR_SERVICE_URL: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
