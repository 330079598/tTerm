import { invoke } from "@tauri-apps/api/core"
import { openUrl } from "@tauri-apps/plugin-opener"
import { useCallback, useState } from "react"
import { useTranslation } from "react-i18next"

import { isTunnelActive, mergeCredentials } from "@/components/TunnelsPanel/tunnelUtils"
import type { useConfirmDialog } from "@/components/ui/app-dialog"
import { useUserVerification } from "@/contexts/UserVerificationContext"
import { useToast } from "@/hooks/use-toast"
import { isVerificationCanceled } from "@/lib/userVerification"
import { toErrorMessage } from "@/lib/utils"
import type {
  CredentialRequest,
  StartOutcome,
  TunnelCredentials,
  TunnelRule,
  TunnelStatus,
} from "@/types/tunnel"

interface TunnelActionsOptions {
  statuses: Record<string, TunnelStatus>
  reload: () => Promise<void>
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
}

/**
 * What a tunnel card's buttons do, and the password prompt a start can need
 * before it succeeds.
 */
export function useTunnelActions({ statuses, reload, confirm }: TunnelActionsOptions) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const { withVaultUnlock } = useUserVerification()
  const [credentialFlow, setCredentialFlow] = useState<{
    rule: TunnelRule
    requests: CredentialRequest[]
    entered: TunnelCredentials
    busy: boolean
  } | null>(null)
  const reportError = useCallback(
    (title: string, error: unknown) =>
      toast({ title, description: toErrorMessage(error), variant: "destructive" }),
    [toast]
  )

  /**
   * Starts a tunnel, asking for any password or key passphrase it is missing.
   * `entered` carries answers from earlier rounds so a wrong passphrase only
   * re-asks for that one secret.
   */
  const startTunnel = useCallback(
    async (rule: TunnelRule, entered: TunnelCredentials = {}) => {
      try {
        const outcome = await withVaultUnlock(() =>
          invoke<StartOutcome>("start_tunnel", { id: rule.id, credentials: entered })
        )
        setCredentialFlow(
          outcome.status === "needsCredentials"
            ? { rule, requests: outcome.requests, entered, busy: false }
            : null
        )
      } catch (error) {
        setCredentialFlow(null)
        if (isVerificationCanceled(error)) return
        reportError(t("tunnels.startFailed", { defaultValue: "Could not start the tunnel" }), error)
      }
    },
    [reportError, t, withVaultUnlock]
  )

  const handleCredentialsSubmit = useCallback(
    async (values: Record<string, string>, remember: boolean) => {
      if (!credentialFlow) return
      const entered = mergeCredentials(
        credentialFlow.entered,
        credentialFlow.requests,
        values,
        remember
      )
      setCredentialFlow({ ...credentialFlow, busy: true })
      await startTunnel(credentialFlow.rule, entered)
    },
    [credentialFlow, startTunnel]
  )

  const handleToggle = useCallback(
    async (rule: TunnelRule, status: TunnelStatus) => {
      if (!isTunnelActive(status.state)) {
        await startTunnel(rule)
        return
      }
      try {
        await invoke("stop_tunnel", { id: rule.id })
      } catch (error) {
        reportError(t("tunnels.stopFailed", { defaultValue: "Could not stop the tunnel" }), error)
      }
    },
    [reportError, startTunnel, t]
  )

  const handleSave = useCallback(
    async (rule: TunnelRule) => {
      const wasActive = isTunnelActive(statuses[rule.id]?.state ?? "stopped")
      await invoke("save_tunnel", { tunnel: rule })
      await reload()
      if (wasActive) {
        // A running tunnel keeps its old settings, so restart it to apply the edit.
        try {
          await invoke("stop_tunnel", { id: rule.id })
        } catch (error) {
          reportError(t("tunnels.stopFailed", { defaultValue: "Could not stop the tunnel" }), error)
          return
        }
        await startTunnel(rule)
      }
    },
    [reload, reportError, startTunnel, statuses, t]
  )

  const handleDelete = useCallback(
    async (rule: TunnelRule) => {
      const confirmed = await confirm({
        title: t("tunnels.deleteConfirmTitle", { defaultValue: "Delete this tunnel?" }),
        description: t("tunnels.deleteConfirmDescription", {
          name: rule.name,
          defaultValue: "“{{name}}” will be stopped and removed.",
        }),
        confirmText: t("tunnels.delete", { defaultValue: "Delete" }),
        variant: "destructive",
      })
      if (!confirmed) return
      try {
        await invoke("delete_tunnel", { id: rule.id })
        await reload()
      } catch (error) {
        reportError(t("tunnels.deleteFailed", { defaultValue: "Failed to delete tunnel" }), error)
      }
    },
    [confirm, reload, reportError, t]
  )

  const handleResetTraffic = useCallback(
    async (rule: TunnelRule) => {
      const confirmed = await confirm({
        title: t("tunnels.stats.resetConfirmTitle", { defaultValue: "Reset traffic totals?" }),
        description: t("tunnels.stats.resetConfirmDescription", {
          name: rule.name,
          defaultValue: "The all-time traffic of “{{name}}” will be set to zero.",
        }),
        confirmText: t("tunnels.stats.reset", { defaultValue: "Reset totals" }),
        variant: "destructive",
      })
      if (!confirmed) return
      try {
        await invoke("reset_tunnel_traffic_totals", { id: rule.id })
        // A running tunnel reports its new totals itself; a stopped one does not.
        await reload()
      } catch (error) {
        reportError(
          t("tunnels.stats.resetFailed", { defaultValue: "Could not reset the traffic totals" }),
          error
        )
      }
    },
    [confirm, reload, reportError, t]
  )

  const handleCopy = useCallback(
    async (address: string) => {
      try {
        await invoke("plugin:clipboard-manager|write_text", { text: address })
      } catch {
        try {
          await navigator.clipboard.writeText(address)
        } catch (error) {
          reportError(t("tunnels.copyFailed", { defaultValue: "Could not copy" }), error)
          return
        }
      }
      toast({ title: t("tunnels.copied", { address, defaultValue: "Copied {{address}}" }) })
    },
    [reportError, t, toast]
  )

  const handleOpen = useCallback(
    (url: string) => {
      openUrl(url).catch((error) =>
        reportError(t("tunnels.openFailed", { defaultValue: "Could not open the browser" }), error)
      )
    },
    [reportError, t]
  )

  return {
    credentialFlow,
    cancelCredentials: () => setCredentialFlow(null),
    handleCredentialsSubmit,
    handleToggle,
    handleSave,
    handleDelete,
    handleResetTraffic,
    handleCopy,
    handleOpen,
  }
}
