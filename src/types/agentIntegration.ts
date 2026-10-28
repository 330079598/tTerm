/** `AgentIntegrationStatus` (src-tauri/src/agent_integration). */
export interface AgentIntegrationStatus {
  agent: "claudeCode" | "codex" | "openCode" | "pi"
  /** Merged into the agent's settings, or a file of tTerm's own. */
  kind: "hooks" | "plugin"
  state: "notInstalled" | "installed" | "outdated"
  detected: boolean
  configDir: string
  configPath: string
  configSource: "setting" | "environment" | "default"
  preview: string
  error: string | null
}
