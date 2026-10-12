import React, { useCallback, useState } from "react"
import { Plus, Waypoints } from "lucide-react"
import { useTranslation } from "react-i18next"

import { TunnelCard } from "@/components/TunnelsPanel/TunnelCard"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Button } from "@/components/ui/button"
import type { TunnelRule } from "@/types/tunnel"
import { CredentialsDialog } from "@/components/TunnelsPanel/CredentialsDialog"
import { TunnelDialog } from "@/components/TunnelsPanel/TunnelDialog"
import { profileLabel, stoppedStatus } from "@/components/TunnelsPanel/tunnelUtils"
import { useTunnelActions } from "@/components/TunnelsPanel/useTunnelActions"
import { useTunnelList } from "@/components/TunnelsPanel/useTunnelList"

interface TunnelsPanelProps {
  profilesRefreshKey?: number
}

export const TunnelsPanel: React.FC<TunnelsPanelProps> = ({ profilesRefreshKey }) => {
  const { t } = useTranslation()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const { rules, profiles, statuses, loading, now, sshProfiles, profilesById, reload } =
    useTunnelList(profilesRefreshKey)
  const {
    credentialFlow,
    cancelCredentials,
    handleCredentialsSubmit,
    handleToggle,
    handleSave,
    handleDelete,
    handleResetTraffic,
    handleCopy,
    handleOpen,
  } = useTunnelActions({ statuses, reload, confirm })
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editing, setEditing] = useState<TunnelRule | null>(null)

  const openCreate = () => {
    setEditing(null)
    setDialogOpen(true)
  }
  const openEdit = useCallback((rule: TunnelRule) => {
    setEditing(rule)
    setDialogOpen(true)
  }, [])

  const runningCount = rules.filter((rule) => statuses[rule.id]?.state === "running").length

  return (
    <section className="flex h-full min-h-0 flex-col">
      <header className="border-border/80 flex items-start justify-between gap-4 border-b px-6 py-4">
        <div className="min-w-0">
          <h2 className="text-base font-semibold tracking-[-0.02em]">
            {t("tunnels.title", { defaultValue: "Port Forwarding" })}
          </h2>
          <p className="text-muted-foreground mt-1 text-sm">
            {t("tunnels.description", {
              defaultValue: "Reach private services through SSH, or expose local ones to a server.",
            })}
            {runningCount > 0 && (
              <span className="text-success ml-2 font-medium">
                {t("tunnels.runningCount", {
                  count: runningCount,
                  defaultValue: "{{count}} running",
                })}
              </span>
            )}
          </p>
        </div>
        <Button type="button" onClick={openCreate}>
          <Plus />
          {t("tunnels.new", { defaultValue: "New tunnel" })}
        </Button>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto p-6">
        {loading ? null : rules.length === 0 ? (
          <div className="mx-auto flex max-w-md flex-col items-center gap-3 py-16 text-center">
            <div className="bg-muted text-muted-foreground flex size-12 items-center justify-center rounded-xl border">
              <Waypoints className="size-6" />
            </div>
            <h3 className="text-base font-semibold">
              {t("tunnels.emptyTitle", { defaultValue: "No tunnels yet" })}
            </h3>
            <p className="text-muted-foreground text-sm">
              {t("tunnels.emptyDescription", {
                defaultValue:
                  "Create a local, remote or dynamic (SOCKS5) forward and switch it on whenever you need it.",
              })}
            </p>
            <Button type="button" onClick={openCreate}>
              <Plus />
              {t("tunnels.new", { defaultValue: "New tunnel" })}
            </Button>
          </div>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,380px),1fr))] gap-4">
            {rules.map((rule) => {
              const profile = profilesById.get(rule.profileId)
              return (
                <TunnelCard
                  key={rule.id}
                  rule={rule}
                  status={statuses[rule.id] ?? stoppedStatus(rule.id)}
                  hostLabel={
                    profileLabel(profile) || t("tunnels.route.server", { defaultValue: "SSH host" })
                  }
                  hostMissing={profiles.length > 0 && !profile}
                  now={now}
                  onToggle={handleToggle}
                  onCopy={handleCopy}
                  onOpen={handleOpen}
                  onEdit={openEdit}
                  onDelete={handleDelete}
                  onResetTraffic={handleResetTraffic}
                />
              )
            })}
          </div>
        )}
      </div>

      <TunnelDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        tunnel={editing}
        profiles={sshProfiles}
        onSave={handleSave}
      />
      {credentialFlow && (
        <CredentialsDialog
          key={credentialFlow.requests
            .map((request) => `${request.hop}:${request.kind}:${request.incorrect}`)
            .join("|")}
          tunnelName={credentialFlow.rule.name}
          requests={credentialFlow.requests}
          busy={credentialFlow.busy}
          onSubmit={handleCredentialsSubmit}
          onCancel={cancelCredentials}
        />
      )}
      <ConfirmDialog />
    </section>
  )
}
