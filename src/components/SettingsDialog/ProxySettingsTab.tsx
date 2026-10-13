import React, { useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { Network, Pencil, Plus, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ProxyServerDialog } from "@/components/SettingsDialog/ProxyServerDialog"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import type { useConfirmDialog } from "@/components/ui/app-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select } from "@/components/ui/select"
import { useConfig } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"
import { useProxyServers } from "@/hooks/useProxyServers"
import { countProfilesUsingProxy, proxySummary } from "@/lib/proxy"
import { toErrorMessage } from "@/lib/utils"
import type { ProxyServerEntry } from "@/types/proxy"
import type { SavedProfile } from "@/types/tab"

interface ProxySettingsTabProps {
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
}

/** The saved proxy servers and which of them hosts use by default. */
export const ProxySettingsTab: React.FC<ProxySettingsTabProps> = ({ confirm }) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const { config, saveConfig } = useConfig()
  const { servers, loaded, loadError, save, remove } = useProxyServers()
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editing, setEditing] = useState<ProxyServerEntry | null>(null)
  const [deletingId, setDeletingId] = useState<string | null>(null)

  const defaultId = config.default_proxy_id
  const defaultMissing = loaded && defaultId !== "" && !servers.some((s) => s.id === defaultId)

  const openEditor = (server: ProxyServerEntry | null) => {
    setEditing(server)
    setDialogOpen(true)
  }

  const handleDefaultChange = async (id: string) => {
    try {
      await saveConfig({ default_proxy_id: id })
    } catch (error) {
      toast({
        title: t("errors.configSaveFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    }
  }

  const handleDelete = async (server: ProxyServerEntry) => {
    const profiles = await invoke<SavedProfile[]>("list_profiles").catch(() => [])
    const count = countProfilesUsingProxy(profiles, server.id)
    const confirmed = await confirm({
      title: t("proxies.deleteTitle"),
      description:
        count > 0
          ? t("proxies.deleteConfirmInUse", { name: server.name, count })
          : t("proxies.deleteConfirm", { name: server.name }),
      confirmText: t("common.delete"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })
    if (!confirmed) return

    setDeletingId(server.id)
    try {
      await remove(server.id)
      if (defaultId === server.id) {
        await saveConfig({ default_proxy_id: "" })
      }
    } catch (error) {
      toast({
        title: t("proxies.deleteFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setDeletingId(null)
    }
  }

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <SettingsSection
          icon={<Network size={16} />}
          title={t("proxies.title")}
          description={t("proxies.description")}
        >
          <SettingsRow
            title={t("proxies.defaultTitle")}
            description={
              defaultMissing ? (
                <span className="text-destructive">{t("proxies.defaultMissing")}</span>
              ) : (
                t("proxies.defaultDescription")
              )
            }
            action={
              <Select
                aria-label={t("proxies.defaultTitle")}
                className="w-56"
                value={defaultId}
                onChange={(event) => void handleDefaultChange(event.target.value)}
              >
                <option value="">{t("proxies.defaultNone")}</option>
                {servers.map((server) => (
                  <option key={server.id} value={server.id}>
                    {server.name}
                  </option>
                ))}
                {defaultMissing && <option value={defaultId}>{t("proxies.deletedOption")}</option>}
              </Select>
            }
          />

          <Card>
            <CardContent className="space-y-3 p-4">
              <div className="flex items-center justify-between gap-3">
                <div className="text-sm font-medium">{t("proxies.listTitle")}</div>
                <Button size="sm" variant="outline" onClick={() => openEditor(null)}>
                  <Plus size={14} />
                  {t("proxies.add")}
                </Button>
              </div>

              {loadError ? (
                <p className="text-destructive text-xs leading-5" role="alert">
                  {t("proxies.loadFailed", { error: loadError })}
                </p>
              ) : servers.length === 0 ? (
                <p className="text-muted-foreground text-xs leading-5">{t("proxies.empty")}</p>
              ) : (
                <div className="border-border divide-border divide-y rounded-md border">
                  {servers.map((server) => (
                    <div
                      key={server.id}
                      className="flex items-center justify-between gap-3 px-3 py-2"
                    >
                      <div className="min-w-0">
                        <div className="flex min-w-0 items-center gap-2">
                          <span className="truncate text-sm">{server.name}</span>
                          {server.id === defaultId && (
                            <Badge variant="secondary" className="h-5 shrink-0 px-1.5 text-[10px]">
                              {t("proxies.defaultBadge")}
                            </Badge>
                          )}
                        </div>
                        <div className="text-muted-foreground truncate font-mono text-xs">
                          {proxySummary(server)}
                          {server.username ? ` · ${server.username}` : ""}
                        </div>
                      </div>
                      <div className="flex shrink-0 items-center gap-1">
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          onClick={() => openEditor(server)}
                          title={t("proxies.edit")}
                          aria-label={t("proxies.editLabel", { name: server.name })}
                        >
                          <Pencil size={14} />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          className="text-destructive hover:text-destructive"
                          disabled={deletingId !== null}
                          onClick={() => void handleDelete(server)}
                          title={t("proxies.delete")}
                          aria-label={t("proxies.deleteLabel", { name: server.name })}
                        >
                          <Trash2 size={14} />
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
              <p className="text-muted-foreground text-xs leading-5">{t("proxies.firstHopNote")}</p>
            </CardContent>
          </Card>
        </SettingsSection>
      </div>

      <ProxyServerDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        server={editing}
        onSave={save}
      />
    </ScrollArea>
  )
}
