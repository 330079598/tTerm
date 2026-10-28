import React, { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import { AgentIntegrationRow } from "@/components/AgentIntegrationRow"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { toast } from "@/hooks/use-toast"
import { toErrorMessage } from "@/lib/utils"
import type { AgentIntegrationStatus } from "@/types/agentIntegration"
import type { Tab } from "@/types/tab"

interface RemoteAgentIntegrationDialogProps {
  tabId: string
  connection: Tab["connection"]
  onClose: () => void
}

type Loaded =
  | { kind: "loading" }
  | { kind: "failed"; error: string }
  | { kind: "loaded"; statuses: AgentIntegrationStatus[] }

/**
 * Installs the hooks that let AI agents report to tTerm on the tab's SSH
 * host, for agents run there. Mounted while open, so the host is read again
 * each time.
 */
export const RemoteAgentIntegrationDialog: React.FC<RemoteAgentIntegrationDialogProps> = ({
  tabId,
  connection,
  onClose,
}) => {
  const { t } = useTranslation()
  const [loaded, setLoaded] = useState<Loaded>({ kind: "loading" })
  const [busyAgent, setBusyAgent] = useState<string | null>(null)
  const [attempt, setAttempt] = useState(0)

  useEffect(() => {
    let current = true
    invoke<AgentIntegrationStatus[]>("remote_agent_integration_status", { tabId, connection })
      .then((statuses) => current && setLoaded({ kind: "loaded", statuses }))
      .catch((error: unknown) => {
        if (current) setLoaded({ kind: "failed", error: toErrorMessage(error) })
      })
    return () => {
      current = false
    }
  }, [attempt, connection, tabId])

  const retry = () => {
    setLoaded({ kind: "loading" })
    setAttempt(attempt + 1)
  }

  const change = useCallback(
    async (status: AgentIntegrationStatus, install: boolean) => {
      setBusyAgent(status.agent)
      try {
        const next = await invoke<AgentIntegrationStatus>("change_remote_agent_integration", {
          tabId,
          connection,
          agent: status.agent,
          install,
        })
        setLoaded((current) =>
          current.kind === "loaded"
            ? {
                kind: "loaded",
                statuses: current.statuses.map((item) => (item.agent === next.agent ? next : item)),
              }
            : current
        )
      } catch (error) {
        toast({
          title: install
            ? t("notifications.agents.installFailed")
            : t("notifications.agents.uninstallFailed"),
          description: toErrorMessage(error),
          variant: "destructive",
        })
      } finally {
        setBusyAgent(null)
      }
    },
    [connection, t, tabId]
  )

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[85vh] max-w-2xl overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{t("remoteAgents.title", { host: connection?.host ?? "" })}</DialogTitle>
          <DialogDescription>{t("remoteAgents.description")}</DialogDescription>
        </DialogHeader>
        {loaded.kind === "loading" && (
          <p className="text-muted-foreground text-sm">{t("remoteAgents.loading")}</p>
        )}
        {loaded.kind === "failed" && (
          <div className="space-y-3">
            <p className="text-destructive text-sm">
              {t("remoteAgents.loadFailed", { error: loaded.error })}
            </p>
            <Button type="button" variant="outline" onClick={retry}>
              {t("remoteAgents.retry")}
            </Button>
          </div>
        )}
        {loaded.kind === "loaded" && (
          <div className="space-y-2">
            {loaded.statuses.map((status) => (
              <AgentIntegrationRow
                key={status.agent}
                status={status}
                place="remote"
                busy={busyAgent === status.agent}
                onChange={(install) => void change(status, install)}
              />
            ))}
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
