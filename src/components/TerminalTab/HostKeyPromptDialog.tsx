import React from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import { Crab } from "@/components/Crab"
import { HostKeyPromptState } from "@/components/TerminalTab/types"

interface HostKeyPromptDialogProps {
  hostKeyPrompt: HostKeyPromptState | null
  setHostKeyPrompt: (value: HostKeyPromptState | null) => void
}

export const HostKeyPromptDialog: React.FC<HostKeyPromptDialogProps> = ({
  hostKeyPrompt,
  setHostKeyPrompt,
}) => {
  const { t } = useTranslation()

  if (!hostKeyPrompt) {
    return null
  }

  const mismatch = hostKeyPrompt.reason === "mismatch"

  return (
    <div className="host-key-dialog-overlay">
      <div className="host-key-dialog-content">
        <div style={{ display: "flex", alignItems: "center", gap: 12, margin: "0 0 12px" }}>
          {/* A new host knocks to be let in, afresh for each host in a queue of
              prompts. A changed key may be an attack, which is no time for a
              playful crab. */}
          {!mismatch && <Crab key={hostKeyPrompt.requestId} motion="knock" height={40} />}
          <h3
            style={{
              margin: 0,
              fontSize: "calc(16px * var(--ui-font-scale, 1))",
              fontWeight: 600,
            }}
          >
            {mismatch ? t("ssh.hostKeyMismatch") : t("ssh.unknownHostKey")}
          </h3>
        </div>
        <div
          style={{
            fontSize: "calc(13px * var(--ui-font-scale, 1))",
            lineHeight: 1.6,
            marginBottom: 16,
            fontFamily: "monospace",
            background: "hsl(var(--muted))",
            padding: "10px 12px",
            borderRadius: 4,
          }}
        >
          <div>
            <b>{t("ssh.host")}:</b> {hostKeyPrompt.host}:{hostKeyPrompt.port}
          </div>
          <div>
            <b>{t("ssh.algorithm")}:</b> {hostKeyPrompt.algorithm}
          </div>
          <div>
            <b>{t("ssh.fingerprint")}:</b> {hostKeyPrompt.fingerprint}
          </div>
          {hostKeyPrompt.knownFingerprint && (
            <div style={{ color: "hsl(var(--destructive))" }}>
              <b>{t("ssh.knownFingerprint")}:</b> {hostKeyPrompt.knownFingerprint}
            </div>
          )}
        </div>
        <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
          <button
            onClick={async () => {
              await invoke("respond_ssh_host_key_prompt", {
                requestId: hostKeyPrompt.requestId,
                trust: false,
              }).catch(console.error)
              setHostKeyPrompt(null)
            }}
            style={{
              padding: "6px 16px",
              borderRadius: 4,
              border: "1px solid hsl(var(--border))",
              background: "hsl(var(--muted))",
              color: "hsl(var(--foreground))",
              cursor: "pointer",
            }}
          >
            {t("ssh.reject")}
          </button>
          <button
            onClick={async () => {
              await invoke("respond_ssh_host_key_prompt", {
                requestId: hostKeyPrompt.requestId,
                trust: true,
              }).catch(console.error)
              setHostKeyPrompt(null)
            }}
            style={{
              padding: "6px 16px",
              borderRadius: 4,
              border: "none",
              background: "hsl(var(--primary))",
              color: "hsl(var(--primary-foreground))",
              cursor: "pointer",
            }}
          >
            {t("ssh.trust")}
          </button>
        </div>
      </div>
    </div>
  )
}
